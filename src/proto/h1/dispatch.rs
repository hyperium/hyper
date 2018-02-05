use std::io;

use futures::{Async, AsyncSink, Future, Poll, Stream};
use futures::sync::oneshot;
use tokio_io::{AsyncRead, AsyncWrite};
use tokio_service::Service;

use proto::{Body, Conn, KeepAlive, Http1Transaction, MessageHead, RequestHead, ResponseHead};
use ::StatusCode;

pub struct Dispatcher<D, Bs, I, B, T, K> {
    conn: Conn<I, B, T, K>,
    dispatch: D,
    body_tx: Option<::proto::body::ChunkSender>,
    body_rx: Option<Bs>,
    is_closing: bool,
}

pub trait Dispatch {
    type PollItem;
    type PollBody;
    type RecvItem;
    fn poll_msg(&mut self) -> Poll<Option<(Self::PollItem, Option<Self::PollBody>)>, ::Error>;
    fn recv_msg(&mut self, msg: ::Result<(Self::RecvItem, Option<Body>)>) -> ::Result<()>;
    fn poll_ready(&mut self) -> Poll<(), ()>;
    fn should_poll(&self) -> bool;
}

pub struct Server<S: Service> {
    in_flight: Option<S::Future>,
    service: S,
}

pub struct Client<B> {
    callback: Option<oneshot::Sender<::Result<::Response>>>,
    rx: ClientRx<B>,
}

pub type ClientMsg<B> = (RequestHead, Option<B>);

type ClientRx<B> = ::client::dispatch::Receiver<ClientMsg<B>, ::Response>;

impl<D, Bs, I, B, T, K> Dispatcher<D, Bs, I, B, T, K>
where
    D: Dispatch<PollItem=MessageHead<T::Outgoing>, PollBody=Bs, RecvItem=MessageHead<T::Incoming>>,
    I: AsyncRead + AsyncWrite,
    B: AsRef<[u8]>,
    T: Http1Transaction,
    K: KeepAlive,
    Bs: Stream<Item=B, Error=::Error>,
{
    pub fn new(dispatch: D, conn: Conn<I, B, T, K>) -> Self {
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

    fn poll2(&mut self) -> Poll<(), ::Error> {
        self.poll_read()?;
        self.poll_write()?;
        self.poll_flush()?;

        if self.is_done() {
            try_ready!(self.conn.shutdown());
            self.conn.take_error()?;
            trace!("Dispatch::poll done");
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
                            match body.start_send(Ok(chunk)) {
                                Ok(AsyncSink::Ready) => {
                                    self.body_tx = Some(body);
                                },
                                Ok(AsyncSink::NotReady(_chunk)) => {
                                    unreachable!("mpsc poll_ready was ready, start_send was not");
                                }
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
                            let _ = body.start_send(Err(::Error::Io(e)));
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
                    let (mut tx, rx) = ::proto::body::channel();
                    let _ = tx.poll_ready(); // register this task if rx is dropped
                    self.body_tx = Some(tx);
                    Some(rx)
                } else {
                    None
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
            } else if self.body_rx.is_none() && self.dispatch.should_poll() {
                if let Some((head, body)) = try_ready!(self.dispatch.poll_msg()) {
                    self.conn.write_head(head, body.is_some());
                    self.body_rx = body;
                } else {
                    self.close();
                    return Ok(Async::Ready(()));
                }
            } else if !self.conn.can_buffer_body() {
                try_ready!(self.poll_flush());
            } else if let Some(mut body) = self.body_rx.take() {
                let chunk = match body.poll()? {
                    Async::Ready(Some(chunk)) => {
                        self.body_rx = Some(body);
                        chunk
                    },
                    Async::Ready(None) => {
                        if self.conn.can_write_body() {
                            self.conn.write_body(None)?;
                        }
                        continue;
                    },
                    Async::NotReady => {
                        self.body_rx = Some(body);
                        return Ok(Async::NotReady);
                    }
                };

                if self.conn.can_write_body() {
                    assert!(self.conn.write_body(Some(chunk))?.is_ready());
                // This allows when chunk is `None`, or `Some([])`.
                } else if chunk.as_ref().len() == 0 {
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
            err.into()
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


impl<D, Bs, I, B, T, K> Future for Dispatcher<D, Bs, I, B, T, K>
where
    D: Dispatch<PollItem=MessageHead<T::Outgoing>, PollBody=Bs, RecvItem=MessageHead<T::Incoming>>,
    I: AsyncRead + AsyncWrite,
    B: AsRef<[u8]>,
    T: Http1Transaction,
    K: KeepAlive,
    Bs: Stream<Item=B, Error=::Error>,
{
    type Item = ();
    type Error = ::Error;

    #[inline]
    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        trace!("Dispatcher::poll");
        self.poll2().or_else(|e| {
            // An error means we're shutting down either way.
            // We just try to give the error to the user,
            // and close the connection with an Ok. If we
            // cannot give it to the user, then return the Err.
            self.dispatch.recv_msg(Err(e)).map(Async::Ready)
        })
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
    S: Service<Request=::Request, Response=::Response<Bs>, Error=::Error>,
    Bs: Stream<Error=::Error>,
    Bs::Item: AsRef<[u8]>,
{
    type PollItem = MessageHead<StatusCode>;
    type PollBody = Bs;
    type RecvItem = RequestHead;

    fn poll_msg(&mut self) -> Poll<Option<(Self::PollItem, Option<Self::PollBody>)>, ::Error> {
        if let Some(mut fut) = self.in_flight.take() {
            let resp = match fut.poll()? {
                Async::Ready(res) => res,
                Async::NotReady => {
                    self.in_flight = Some(fut);
                    return Ok(Async::NotReady);
                }
            };
            let (head, body) = ::proto::response::split(resp);
            Ok(Async::Ready(Some((head.into(), body))))
        } else {
            unreachable!("poll_msg shouldn't be called if no inflight");
        }
    }

    fn recv_msg(&mut self, msg: ::Result<(Self::RecvItem, Option<Body>)>) -> ::Result<()> {
        let (msg, body) = msg?;
        let req = ::proto::request::from_wire(None, msg, body);
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
    B: Stream<Error=::Error>,
    B::Item: AsRef<[u8]>,
{
    type PollItem = RequestHead;
    type PollBody = B;
    type RecvItem = ResponseHead;

    fn poll_msg(&mut self) -> Poll<Option<(Self::PollItem, Option<Self::PollBody>)>, ::Error> {
        match self.rx.poll() {
            Ok(Async::Ready(Some(((head, body), mut cb)))) => {
                // check that future hasn't been canceled already
                match cb.poll_cancel().expect("poll_cancel cannot error") {
                    Async::Ready(()) => {
                        trace!("request canceled");
                        Ok(Async::Ready(None))
                    },
                    Async::NotReady => {
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
            Err(_) => unreachable!("receiver cannot error"),
        }
    }

    fn recv_msg(&mut self, msg: ::Result<(Self::RecvItem, Option<Body>)>) -> ::Result<()> {
        match msg {
            Ok((msg, body)) => {
                if let Some(cb) = self.callback.take() {
                    let res = ::proto::response::from_wire(msg, body);
                    let _ = cb.send(Ok(res));
                    Ok(())
                } else {
                    Err(::Error::Io(io::Error::new(io::ErrorKind::InvalidData, "response received without matching request")))
                }
            },
            Err(err) => {
                if let Some(cb) = self.callback.take() {
                    let _ = cb.send(Err(err));
                    Ok(())
                } else if let Ok(Async::Ready(Some((_, cb)))) = self.rx.poll() {
                    let _ = cb.send(Err(err));
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
    use super::*;
    use mock::AsyncIo;
    use proto::ClientTransaction;

    #[test]
    fn client_read_response_before_writing_request() {
        extern crate pretty_env_logger;
        let _ = pretty_env_logger::try_init();
        ::futures::lazy(|| {
            let io = AsyncIo::new_buf(b"HTTP/1.1 200 OK\r\n\r\n".to_vec(), 100);
            let (tx, rx) = ::client::dispatch::channel();
            let conn = Conn::<_, ::Chunk, ClientTransaction>::new(io, Default::default());
            let mut dispatcher = Dispatcher::new(Client::new(rx), conn);

            let req = RequestHead {
                version: ::HttpVersion::Http11,
                subject: ::proto::RequestLine::default(),
                headers: Default::default(),
            };
            let res_rx = tx.send((req, None::<::Body>)).unwrap();

            dispatcher.poll().expect("dispatcher poll 1");
            dispatcher.poll().expect("dispatcher poll 2");
            let _res = res_rx.wait()
                .expect("callback poll")
                .expect("callback response");
            Ok::<(), ()>(())
        }).wait().unwrap();
    }
}
