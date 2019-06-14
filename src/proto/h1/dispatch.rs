use std::error::Error as StdError;

use bytes::{Buf, Bytes};
use futures::{Async, Future, Poll, Stream};
use http::{Request, Response, StatusCode};
use tokio_io::{AsyncRead, AsyncWrite};

use body::{Body, Payload};
use body::internal::FullDataArg;
use common::{Never, YieldNow};
use proto::{BodyLength, DecodedLength, Conn, Dispatched, MessageHead, RequestHead, RequestLine, ResponseHead};
use super::Http1Transaction;
use service::Service;

pub(crate) struct Dispatcher<D, Bs: Payload, I, T> {
    conn: Conn<I, Bs::Data, T>,
    dispatch: D,
    body_tx: Option<::body::Sender>,
    body_rx: Option<Bs>,
    is_closing: bool,
    /// If the poll loop reaches its max spin count, it will yield by notifying
    /// the task immediately. This will cache that `Task`, since it usually is
    /// the same one.
    yield_now: YieldNow,
}

pub(crate) trait Dispatch {
    type PollItem;
    type PollBody;
    type PollError;
    type RecvItem;
    fn poll_msg(&mut self) -> Poll<Option<(Self::PollItem, Self::PollBody)>, Self::PollError>;
    fn recv_msg(&mut self, msg: ::Result<(Self::RecvItem, Body)>) -> ::Result<()>;
    fn poll_ready(&mut self) -> Poll<(), ()>;
    fn should_poll(&self) -> bool;
}

pub struct Server<S: Service> {
    in_flight: Option<S::Future>,
    pub(crate) service: S,
}

pub struct Client<B> {
    callback: Option<::client::dispatch::Callback<Request<B>, Response<Body>>>,
    rx: ClientRx<B>,
}

type ClientRx<B> = ::client::dispatch::Receiver<Request<B>, Response<Body>>;

impl<D, Bs, I, T> Dispatcher<D, Bs, I, T>
where
    D: Dispatch<PollItem=MessageHead<T::Outgoing>, PollBody=Bs, RecvItem=MessageHead<T::Incoming>>,
    D::PollError: Into<Box<dyn StdError + Send + Sync>>,
    I: AsyncRead + AsyncWrite,
    T: Http1Transaction,
    Bs: Payload,
{
    pub fn new(dispatch: D, conn: Conn<I, Bs::Data, T>) -> Self {
        Dispatcher {
            conn: conn,
            dispatch: dispatch,
            body_tx: None,
            body_rx: None,
            is_closing: false,
            yield_now: YieldNow::new(),
        }
    }

    pub fn disable_keep_alive(&mut self) {
        self.conn.disable_keep_alive()
    }

    pub fn into_inner(self) -> (I, Bytes, D) {
        let (io, buf) = self.conn.into_inner();
        (io, buf, self.dispatch)
    }

    /// Run this dispatcher until HTTP says this connection is done,
    /// but don't call `AsyncWrite::shutdown` on the underlying IO.
    ///
    /// This is useful for old-style HTTP upgrades, but ignores
    /// newer-style upgrade API.
    pub fn poll_without_shutdown(&mut self) -> Poll<(), ::Error> {
        self.poll_catch(false)
            .map(|x| {
                x.map(|ds| if let Dispatched::Upgrade(pending) = ds {
                    pending.manual();
                })
            })
    }

    fn poll_catch(&mut self, should_shutdown: bool) -> Poll<Dispatched, ::Error> {
        self.poll_inner(should_shutdown).or_else(|e| {
            // An error means we're shutting down either way.
            // We just try to give the error to the user,
            // and close the connection with an Ok. If we
            // cannot give it to the user, then return the Err.
            self.dispatch.recv_msg(Err(e))?;
            Ok(Async::Ready(Dispatched::Shutdown))
        })
    }

    fn poll_inner(&mut self, should_shutdown: bool) -> Poll<Dispatched, ::Error> {
        T::update_date();

        try_ready!(self.poll_loop());

        if self.is_done() {
            if let Some(pending) = self.conn.pending_upgrade() {
                self.conn.take_error()?;
                return Ok(Async::Ready(Dispatched::Upgrade(pending)));
            } else if should_shutdown {
                try_ready!(self.conn.shutdown().map_err(::Error::new_shutdown));
            }
            self.conn.take_error()?;
            Ok(Async::Ready(Dispatched::Shutdown))
        } else {
            Ok(Async::NotReady)
        }
    }

    fn poll_loop(&mut self) -> Poll<(), ::Error> {
        // Limit the looping on this connection, in case it is ready far too
        // often, so that other futures don't starve.
        //
        // 16 was chosen arbitrarily, as that is number of pipelined requests
        // benchmarks often use. Perhaps it should be a config option instead.
        for _ in 0..16 {
            self.poll_read()?;
            self.poll_write()?;
            self.poll_flush()?;

            // This could happen if reading paused before blocking on IO,
            // such as getting to the end of a framed message, but then
            // writing/flushing set the state back to Init. In that case,
            // if the read buffer still had bytes, we'd want to try poll_read
            // again, or else we wouldn't ever be woken up again.
            //
            // Using this instead of task::current() and notify() inside
            // the Conn is noticeably faster in pipelined benchmarks.
            if !self.conn.wants_read_again() {
                //break;
                return Ok(Async::Ready(()));
            }
        }

        trace!("poll_loop yielding (self = {:p})", self);

        match self.yield_now.poll_yield() {
            Ok(Async::NotReady) => Ok(Async::NotReady),
            // maybe with `!` this can be cleaner...
            // but for now, just doing this to eliminate branches
            Ok(Async::Ready(never)) |
            Err(never) => match never {}
        }
    }

    fn poll_read(&mut self) -> Poll<(), ::Error> {
        loop {
            if self.is_closing {
                return Ok(Async::Ready(()));
            } else if self.conn.can_read_head() {
                try_ready!(self.poll_read_head());
            } else if let Some(mut body) = self.body_tx.take() {
                if self.conn.can_read_body() {
                    match body.poll_ready() {
                        Ok(Async::Ready(())) => (),
                        Ok(Async::NotReady) => {
                            self.body_tx = Some(body);
                            return Ok(Async::NotReady);
                        },
                        Err(_canceled) => {
                            // user doesn't care about the body
                            // so we should stop reading
                            trace!("body receiver dropped before eof, closing");
                            self.conn.close_read();
                            return Ok(Async::Ready(()));
                        }
                    }
                    match self.conn.read_body() {
                        Ok(Async::Ready(Some(chunk))) => {
                            match body.send_data(chunk) {
                                Ok(()) => {
                                    self.body_tx = Some(body);
                                },
                                Err(_canceled) => {
                                    if self.conn.can_read_body() {
                                        trace!("body receiver dropped before eof, closing");
                                        self.conn.close_read();
                                    }
                                }
                            }
                        },
                        Ok(Async::Ready(None)) => {
                            // just drop, the body will close automatically
                        },
                        Ok(Async::NotReady) => {
                            self.body_tx = Some(body);
                            return Ok(Async::NotReady);
                        }
                        Err(e) => {
                            body.send_error(::Error::new_body(e));
                        }
                    }
                } else {
                    // just drop, the body will close automatically
                }
            } else {
                return self.conn.read_keep_alive();
            }
        }
    }

    fn poll_read_head(&mut self) -> Poll<(), ::Error> {
        // can dispatch receive, or does it still care about, an incoming message?
        match self.dispatch.poll_ready() {
            Ok(Async::Ready(())) => (),
            Ok(Async::NotReady) => return Ok(Async::NotReady), // service might not be ready
            Err(()) => {
                trace!("dispatch no longer receiving messages");
                self.close();
                return Ok(Async::Ready(()));
            }
        }
        // dispatch is ready for a message, try to read one
        match self.conn.read_head() {
            Ok(Async::Ready(Some((head, body_len, wants_upgrade)))) => {
                let mut body = match body_len {
                    DecodedLength::ZERO => Body::empty(),
                    other => {
                        let (tx, rx) = Body::new_channel(other.into_opt());
                        self.body_tx = Some(tx);
                        rx
                    },
                };
                if wants_upgrade {
                    body.set_on_upgrade(self.conn.on_upgrade());
                }
                self.dispatch.recv_msg(Ok((head, body)))?;
                Ok(Async::Ready(()))
            }
            Ok(Async::Ready(None)) => {
                // read eof, conn will start to shutdown automatically
                Ok(Async::Ready(()))
            }
            Ok(Async::NotReady) => Ok(Async::NotReady),
            Err(err) => {
                debug!("read_head error: {}", err);
                self.dispatch.recv_msg(Err(err))?;
                // if here, the dispatcher gave the user the error
                // somewhere else. we still need to shutdown, but
                // not as a second error.
                Ok(Async::Ready(()))
            }
        }
    }

    fn poll_write(&mut self) -> Poll<(), ::Error> {
        loop {
            if self.is_closing {
                return Ok(Async::Ready(()));
            } else if self.body_rx.is_none() && self.conn.can_write_head() && self.dispatch.should_poll() {
                if let Some((head, mut body)) = try_ready!(self.dispatch.poll_msg().map_err(::Error::new_user_service)) {
                    // Check if the body knows its full data immediately.
                    //
                    // If so, we can skip a bit of bookkeeping that streaming
                    // bodies need to do.
                    if let Some(full) = body.__hyper_full_data(FullDataArg(())).0 {
                        self.conn.write_full_msg(head, full);
                        return Ok(Async::Ready(()));
                    }
                    let body_type = if body.is_end_stream() {
                        self.body_rx = None;
                        None
                    } else {
                        let btype = body.content_length()
                            .map(BodyLength::Known)
                            .or_else(|| Some(BodyLength::Unknown));
                        self.body_rx = Some(body);
                        btype
                    };
                    self.conn.write_head(head, body_type);
                } else {
                    self.close();
                    return Ok(Async::Ready(()));
                }
            } else if !self.conn.can_buffer_body() {
                try_ready!(self.poll_flush());
            } else if let Some(mut body) = self.body_rx.take() {
                if !self.conn.can_write_body() {
                    trace!(
                        "no more write body allowed, user body is_end_stream = {}",
                        body.is_end_stream(),
                    );
                    continue;
                }
                match body.poll_data().map_err(::Error::new_user_body)? {
                    Async::Ready(Some(chunk)) => {
                        let eos = body.is_end_stream();
                        if eos {
                            if chunk.remaining() == 0 {
                                trace!("discarding empty chunk");
                                self.conn.end_body();
                            } else {
                                self.conn.write_body_and_end(chunk);
                            }
                        } else {
                            self.body_rx = Some(body);
                            if chunk.remaining() == 0 {
                                trace!("discarding empty chunk");
                                continue;
                            }
                            self.conn.write_body(chunk);
                        }
                    },
                    Async::Ready(None) => {
                        self.conn.end_body();
                    },
                    Async::NotReady => {
                        self.body_rx = Some(body);
                        return Ok(Async::NotReady);
                    }
                }
            } else {
                return Ok(Async::NotReady);
            }
        }
    }

    fn poll_flush(&mut self) -> Poll<(), ::Error> {
        self.conn.flush().map_err(|err| {
            debug!("error writing: {}", err);
            ::Error::new_body_write(err)
        })
    }

    fn close(&mut self) {
        self.is_closing = true;
        self.conn.close_read();
        self.conn.close_write();
    }

    fn is_done(&self) -> bool {
        if self.is_closing {
            return true;
        }

        let read_done = self.conn.is_read_closed();

        if !T::should_read_first() && read_done {
            // a client that cannot read may was well be done.
            true
        } else {
            let write_done = self.conn.is_write_closed() ||
                (!self.dispatch.should_poll() && self.body_rx.is_none());
            read_done && write_done
        }
    }
}

impl<D, Bs, I, T> Future for Dispatcher<D, Bs, I, T>
where
    D: Dispatch<PollItem=MessageHead<T::Outgoing>, PollBody=Bs, RecvItem=MessageHead<T::Incoming>>,
    D::PollError: Into<Box<dyn StdError + Send + Sync>>,
    I: AsyncRead + AsyncWrite,
    T: Http1Transaction,
    Bs: Payload,
{
    type Item = Dispatched;
    type Error = ::Error;

    #[inline]
    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        self.poll_catch(true)
    }
}

// ===== impl Server =====

impl<S> Server<S> where S: Service {
    pub fn new(service: S) -> Server<S> {
        Server {
            in_flight: None,
            service: service,
        }
    }
    pub fn into_service(self) -> S {
        self.service
    }
}

impl<S, Bs> Dispatch for Server<S>
where
    S: Service<ReqBody=Body, ResBody=Bs>,
    S::Error: Into<Box<dyn StdError + Send + Sync>>,
    Bs: Payload,
{
    type PollItem = MessageHead<StatusCode>;
    type PollBody = Bs;
    type PollError = S::Error;
    type RecvItem = RequestHead;

    fn poll_msg(&mut self) -> Poll<Option<(Self::PollItem, Self::PollBody)>, Self::PollError> {
        if let Some(mut fut) = self.in_flight.take() {
            let resp = match fut.poll()? {
                Async::Ready(res) => res,
                Async::NotReady => {
                    self.in_flight = Some(fut);
                    return Ok(Async::NotReady);
                }
            };
            let (parts, body) = resp.into_parts();
            let head = MessageHead {
                version: parts.version,
                subject: parts.status,
                headers: parts.headers,
            };
            Ok(Async::Ready(Some((head, body))))
        } else {
            unreachable!("poll_msg shouldn't be called if no inflight");
        }
    }

    fn recv_msg(&mut self, msg: ::Result<(Self::RecvItem, Body)>) -> ::Result<()> {
        let (msg, body) = msg?;
        let mut req = Request::new(body);
        *req.method_mut() = msg.subject.0;
        *req.uri_mut() = msg.subject.1;
        *req.headers_mut() = msg.headers;
        *req.version_mut() = msg.version;
        self.in_flight = Some(self.service.call(req));
        Ok(())
    }

    fn poll_ready(&mut self) -> Poll<(), ()> {
        if self.in_flight.is_some() {
            Ok(Async::NotReady)
        } else {
            self.service.poll_ready()
                .map_err(|_e| {
                    // FIXME: return error value.
                    trace!("service closed");
                })
        }
    }

    fn should_poll(&self) -> bool {
        self.in_flight.is_some()
    }
}

// ===== impl Client =====


impl<B> Client<B> {
    pub fn new(rx: ClientRx<B>) -> Client<B> {
        Client {
            callback: None,
            rx: rx,
        }
    }
}

impl<B> Dispatch for Client<B>
where
    B: Payload,
{
    type PollItem = RequestHead;
    type PollBody = B;
    type PollError = Never;
    type RecvItem = ResponseHead;

    fn poll_msg(&mut self) -> Poll<Option<(Self::PollItem, Self::PollBody)>, Never> {
        match self.rx.poll() {
            Ok(Async::Ready(Some((req, mut cb)))) => {
                // check that future hasn't been canceled already
                match cb.poll_cancel().expect("poll_cancel cannot error") {
                    Async::Ready(()) => {
                        trace!("request canceled");
                        Ok(Async::Ready(None))
                    },
                    Async::NotReady => {
                        let (parts, body) = req.into_parts();
                        let head = RequestHead {
                            version: parts.version,
                            subject: RequestLine(parts.method, parts.uri),
                            headers: parts.headers,
                        };
                        self.callback = Some(cb);
                        Ok(Async::Ready(Some((head, body))))
                    }
                }
            },
            Ok(Async::Ready(None)) => {
                trace!("client tx closed");
                // user has dropped sender handle
                Ok(Async::Ready(None))
            },
            Ok(Async::NotReady) => Ok(Async::NotReady),
            Err(never) => match never {},
        }
    }

    fn recv_msg(&mut self, msg: ::Result<(Self::RecvItem, Body)>) -> ::Result<()> {
        match msg {
            Ok((msg, body)) => {
                if let Some(cb) = self.callback.take() {
                    let mut res = Response::new(body);
                    *res.status_mut() = msg.subject;
                    *res.headers_mut() = msg.headers;
                    *res.version_mut() = msg.version;
                    let _ = cb.send(Ok(res));
                    Ok(())
                } else {
                    // Getting here is likely a bug! An error should have happened
                    // in Conn::require_empty_read() before ever parsing a
                    // full message!
                    Err(::Error::new_unexpected_message())
                }
            },
            Err(err) => {
                if let Some(cb) = self.callback.take() {
                    let _ = cb.send(Err((err, None)));
                    Ok(())
                } else if let Ok(Async::Ready(Some((req, cb)))) = self.rx.poll() {
                    trace!("canceling queued request with connection error: {}", err);
                    // in this case, the message was never even started, so it's safe to tell
                    // the user that the request was completely canceled
                    let _ = cb.send(Err((::Error::new_canceled().with(err), Some(req))));
                    Ok(())
                } else {
                    Err(err)
                }
            }
        }
    }

    fn poll_ready(&mut self) -> Poll<(), ()> {
        match self.callback {
            Some(ref mut cb) => match cb.poll_cancel() {
                Ok(Async::Ready(())) => {
                    trace!("callback receiver has dropped");
                    Err(())
                },
                Ok(Async::NotReady) => Ok(Async::Ready(())),
                Err(_) => unreachable!("oneshot poll_cancel cannot error"),
            },
            None => Err(()),
        }
    }

    fn should_poll(&self) -> bool {
        self.callback.is_none()
    }
}

#[cfg(test)]
mod tests {
    extern crate pretty_env_logger;

    use super::*;
    use mock::AsyncIo;
    use proto::h1::ClientTransaction;

    #[test]
    fn client_read_bytes_before_writing_request() {
        let _ = pretty_env_logger::try_init();
        ::futures::lazy(|| {
            // Block at 0 for now, but we will release this response before
            // the request is ready to write later...
            let io = AsyncIo::new_buf(b"HTTP/1.1 200 OK\r\n\r\n".to_vec(), 0);
            let (mut tx, rx) = ::client::dispatch::channel();
            let conn = Conn::<_, ::Chunk, ClientTransaction>::new(io);
            let mut dispatcher = Dispatcher::new(Client::new(rx), conn);

            // First poll is needed to allow tx to send...
            assert!(dispatcher.poll().expect("nothing is ready").is_not_ready());
            // Unblock our IO, which has a response before we've sent request!
            dispatcher.conn.io_mut().block_in(100);

            let res_rx = tx.try_send(::Request::new(::Body::empty())).unwrap();

            let a1 = dispatcher.poll().expect("error should be sent on channel");
            assert!(a1.is_ready(), "dispatcher should be closed");
            let err = res_rx.wait()
                .expect("callback poll")
                .expect_err("callback response");

            match (err.0.kind(), err.1) {
                (&::error::Kind::Canceled, Some(_)) => (),
                other => panic!("expected Canceled, got {:?}", other),
            }
            Ok::<(), ()>(())
        }).wait().unwrap();
    }

    #[test]
    fn body_empty_chunks_ignored() {
        let _ = pretty_env_logger::try_init();
        ::futures::lazy(|| {
            let io = AsyncIo::new_buf(vec![], 0);
            let (mut tx, rx) = ::client::dispatch::channel();
            let conn = Conn::<_, ::Chunk, ClientTransaction>::new(io);
            let mut dispatcher = Dispatcher::new(Client::new(rx), conn);

            // First poll is needed to allow tx to send...
            assert!(dispatcher.poll().expect("nothing is ready").is_not_ready());

            let body = ::Body::wrap_stream(::futures::stream::once(Ok::<_, ::Error>("")));

            let _res_rx = tx.try_send(::Request::new(body)).unwrap();

            dispatcher.poll().expect("empty body shouldn't panic");
            Ok::<(), ()>(())
        }).wait().unwrap();
    }
}
