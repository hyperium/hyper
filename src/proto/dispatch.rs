use futures::{Async, AsyncSink, Future, Poll, Sink, Stream};
use futures::sync::{mpsc, oneshot};
use tokio_io::{AsyncRead, AsyncWrite};
use tokio_service::Service;

use super::{Body, Conn, KeepAlive, Http1Transaction, MessageHead, RequestHead, ResponseHead};
use ::StatusCode;

pub struct Dispatcher<D, Bs, I, B, T, K> {
    conn: Conn<I, B, T, K>,
    dispatch: D,
    body_tx: Option<super::body::BodySender>,
    body_rx: Option<Bs>,
}

pub trait Dispatch {
    type PollItem;
    type PollBody;
    type RecvItem;
    fn poll_msg(&mut self) -> Poll<Option<(Self::PollItem, Option<Self::PollBody>)>, ::Error>;
    fn recv_msg(&mut self, msg: ::Result<(Self::RecvItem, Option<Body>)>) -> ::Result<()>;
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

type ClientRx<B> = mpsc::Receiver<(RequestHead, Option<B>, oneshot::Sender<::Result<::Response>>)>;

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
        }
    }

    fn poll_read(&mut self) -> Poll<(), ::Error> {
        loop {
            if self.conn.can_read_head() {
                match self.conn.read_head() {
                    Ok(Async::Ready(Some((head, has_body)))) => {
                        let body = if has_body {
                            let (tx, rx) = super::Body::pair();
                            self.body_tx = Some(tx);
                            Some(rx)
                        } else {
                            None
                        };
                        self.dispatch.recv_msg(Ok((head, body))).expect("recv_msg with Ok shouldn't error");
                    },
                    Ok(Async::Ready(None)) => {
                        // read eof, conn will start to shutdown automatically
                        return Ok(Async::Ready(()));
                    }
                    Ok(Async::NotReady) => return Ok(Async::NotReady),
                    Err(err) => {
                        debug!("read_head error: {}", err);
                        self.dispatch.recv_msg(Err(err))?;
                        // if here, the dispatcher gave the user the error
                        // somewhere else. we still need to shutdown, but
                        // not as a second error.
                        return Ok(Async::Ready(()));
                    }
                }
            } else if let Some(mut body) = self.body_tx.take() {
                let can_read_body = self.conn.can_read_body();
                match body.poll_ready() {
                    Ok(Async::Ready(())) => (),
                    Ok(Async::NotReady) => {
                        self.body_tx = Some(body);
                        return Ok(Async::NotReady);
                    },
                    Err(_canceled) => {
                        // user doesn't care about the body
                        // so we should stop reading
                        if can_read_body {
                            trace!("body receiver dropped before eof, closing");
                            self.conn.close_read();
                            return Ok(Async::Ready(()));
                        }
                    }
                }
                if can_read_body {
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
                            let _ = body.close();
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
                    let _ = body.close();
                }
            } else {
                self.conn.maybe_park_read();
                return Ok(Async::Ready(()));
            }
        }
    }

    fn poll_write(&mut self) -> Poll<(), ::Error> {
        loop {
            if self.body_rx.is_none() && self.dispatch.should_poll() {
                if let Some((head, body)) = try_ready!(self.dispatch.poll_msg()) {
                    self.conn.write_head(head, body.is_some());
                    self.body_rx = body;
                } else {
                    self.conn.close_write();
                    return Ok(Async::Ready(()));
                }
            } else if self.conn.has_queued_body() {
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
                assert!(self.conn.write_body(Some(chunk))?.is_ready());
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

    fn is_done(&self) -> bool {
        let read_done = self.conn.is_read_closed();
        let write_done = self.conn.is_write_closed() ||
            (!self.dispatch.should_poll() && self.body_rx.is_none());

        read_done && write_done
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
        self.poll_read()?;
        self.poll_write()?;
        self.poll_flush()?;

        if self.is_done() {
            try_ready!(self.conn.shutdown());
            trace!("Dispatch::poll done");
            Ok(Async::Ready(()))
        } else {
            Ok(Async::NotReady)
        }
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
            let (head, body) = super::response::split(resp);
            Ok(Async::Ready(Some((head.into(), body))))
        } else {
            unreachable!("poll_msg shouldn't be called if no inflight");
        }
    }

    fn recv_msg(&mut self, msg: ::Result<(Self::RecvItem, Option<Body>)>) -> ::Result<()> {
        let (msg, body) = msg?;
        let req = super::request::from_wire(None, msg, body);
        self.in_flight = Some(self.service.call(req));
        Ok(())
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
            Ok(Async::Ready(Some((head, body, cb)))) => {
                self.callback = Some(cb);
                Ok(Async::Ready(Some((head, body))))
            },
            Ok(Async::Ready(None)) => {
                // user has dropped sender handle
                Ok(Async::Ready(None))
            },
            Ok(Async::NotReady) => return Ok(Async::NotReady),
            Err(()) => unreachable!("mpsc receiver cannot error"),
        }
    }

    fn recv_msg(&mut self, msg: ::Result<(Self::RecvItem, Option<Body>)>) -> ::Result<()> {
        match msg {
            Ok((msg, body)) => {
                let res = super::response::from_wire(msg, body);
                let cb = self.callback.take().expect("recv_msg without callback");
                let _ = cb.send(Ok(res));
                Ok(())
            },
            Err(err) => {
                if let Some(cb) = self.callback.take() {
                    let _ = cb.send(Err(err));
                    Ok(())
                } else {
                    Err(err)
                }
            }
        }
    }

    fn should_poll(&self) -> bool {
        self.callback.is_none()
    }
}
