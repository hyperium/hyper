use bytes::{Buf, Bytes};
use futures::{Async, Future, Poll, Stream};
use http::{Request, Response, StatusCode};
use tokio_io::{AsyncRead, AsyncWrite};

use body::{Body, Payload};
use proto::{BodyLength, Conn, Http1Transaction, MessageHead, RequestHead, RequestLine, ResponseHead};
use service::Service;

pub(crate) struct Dispatcher<D, Bs: Payload, I, T> {
    conn: Conn<I, Bs::Data, T>,
    dispatch: D,
    body_tx: Option<::body::Sender>,
    body_rx: Option<Bs>,
    is_closing: bool,
}

pub(crate) trait Dispatch {
    type PollItem;
    type PollBody;
    type RecvItem;
    fn poll_msg(&mut self) -> Poll<Option<(Self::PollItem, Option<Self::PollBody>)>, ::Error>;
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
        }
    }

    pub fn disable_keep_alive(&mut self) {
        self.conn.disable_keep_alive()
    }

    pub fn into_inner(self) -> (I, Bytes, D) {
        let (io, buf) = self.conn.into_inner();
        (io, buf, self.dispatch)
    }

    /// The "Future" poll function. Runs this dispatcher until the
    /// connection is shutdown, or an error occurs.
    pub fn poll_until_shutdown(&mut self) -> Poll<(), ::Error> {
        self.poll_catch(true)
    }

    /// Run this dispatcher until HTTP says this connection is done,
    /// but don't call `AsyncWrite::shutdown` on the underlying IO.
    ///
    /// This is useful for HTTP upgrades.
    pub fn poll_without_shutdown(&mut self) -> Poll<(), ::Error> {
        self.poll_catch(false)
    }

    fn poll_catch(&mut self, should_shutdown: bool) -> Poll<(), ::Error> {
        self.poll_inner(should_shutdown).or_else(|e| {
            // An error means we're shutting down either way.
            // We just try to give the error to the user,
            // and close the connection with an Ok. If we
            // cannot give it to the user, then return the Err.
            self.dispatch.recv_msg(Err(e)).map(Async::Ready)
        })
    }

    fn poll_inner(&mut self, should_shutdown: bool) -> Poll<(), ::Error> {
        loop {
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
                break;
            }
        }

        if self.is_done() {
            if should_shutdown {
                try_ready!(self.conn.shutdown().map_err(::Error::new_shutdown));
            }
            self.conn.take_error()?;
            Ok(Async::Ready(()))
        } else {
            Ok(Async::NotReady)
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
                return self.conn.read_keep_alive().map(Async::Ready);
            }
        }
    }

    fn poll_read_head(&mut self) -> Poll<(), ::Error> {
        // can dispatch receive, or does it still care about, an incoming message?
        match self.dispatch.poll_ready() {
            Ok(Async::Ready(())) => (),
            Ok(Async::NotReady) => unreachable!("dispatch not ready when conn is"),
            Err(()) => {
                trace!("dispatch no longer receiving messages");
                self.close();
                return Ok(Async::Ready(()));
            }
        }
        // dispatch is ready for a message, try to read one
        match self.conn.read_head() {
            Ok(Async::Ready(Some((head, has_body)))) => {
                let body = if has_body {
                    let (mut tx, rx) = Body::channel();
                    let _ = tx.poll_ready(); // register this task if rx is dropped
                    self.body_tx = Some(tx);
                    rx
                } else {
                    Body::empty()
                };
                self.dispatch.recv_msg(Ok((head, body)))?;
                Ok(Async::Ready(()))
            },
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
                if let Some((head, body)) = try_ready!(self.dispatch.poll_msg()) {
                    let body_type = body.as_ref().map(|body| {
                        body.content_length()
                            .map(BodyLength::Known)
                            .unwrap_or(BodyLength::Unknown)
                    });
                    self.conn.write_head(head, body_type);
                    self.body_rx = body;
                } else {
                    self.close();
                    return Ok(Async::Ready(()));
                }
            } else if !self.conn.can_buffer_body() {
                try_ready!(self.poll_flush());
            } else if let Some(mut body) = self.body_rx.take() {
                let chunk = match body.poll_data().map_err(::Error::new_user_body)? {
                    Async::Ready(Some(chunk)) => {
                        self.body_rx = Some(body);
                        chunk
                    },
                    Async::Ready(None) => {
                        if self.conn.can_write_body() {
                            self.conn.write_body(None);
                        }
                        continue;
                    },
                    Async::NotReady => {
                        self.body_rx = Some(body);
                        return Ok(Async::NotReady);
                    }
                };

                if self.conn.can_write_body() {
                    self.conn.write_body(Some(chunk));
                // This allows when chunk is `None`, or `Some([])`.
                } else if chunk.remaining() == 0 {
                    // ok
                } else {
                    warn!("unexpected chunk when body cannot write");
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
    I: AsyncRead + AsyncWrite,
    T: Http1Transaction,
    Bs: Payload,
{
    type Item = ();
    type Error = ::Error;

    #[inline]
    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        self.poll_until_shutdown()
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
}

impl<S, Bs> Dispatch for Server<S>
where
    S: Service<ReqBody=Body, ResBody=Bs>,
    S::Error: Into<Box<::std::error::Error + Send + Sync>>,
    Bs: Payload,
{
    type PollItem = MessageHead<StatusCode>;
    type PollBody = Bs;
    type RecvItem = RequestHead;

    fn poll_msg(&mut self) -> Poll<Option<(Self::PollItem, Option<Self::PollBody>)>, ::Error> {
        if let Some(mut fut) = self.in_flight.take() {
            let resp = match fut.poll().map_err(::Error::new_user_service)? {
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
            let body = if body.is_end_stream() {
                None
            } else {
                Some(body)
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
            Ok(Async::Ready(()))
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
    type RecvItem = ResponseHead;

    fn poll_msg(&mut self) -> Poll<Option<(Self::PollItem, Option<Self::PollBody>)>, ::Error> {
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

                        let body = if body.is_end_stream() {
                            None
                        } else {
                            Some(body)
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
            Ok(Async::NotReady) => return Ok(Async::NotReady),
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
                    Err(::Error::new_mismatched_response())
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
                    let _ = cb.send(Err((::Error::new_canceled(Some(err)), Some(req))));
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
    use proto::ClientTransaction;

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
}
