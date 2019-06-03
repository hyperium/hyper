use bytes::IntoBuf;
use futures::{Async, Future, Poll, Stream};
use futures::future::{self, Either};
use futures::sync::{mpsc, oneshot};
use h2::client::{Builder, Handshake, SendRequest};
use tokio_io::{AsyncRead, AsyncWrite};

use headers::content_length_parse_all;
use body::Payload;
use ::common::{Exec, Never};
use headers;
use ::proto::Dispatched;
use super::{PipeToSendStream, SendBuf};
use ::{Body, Request, Response};

type ClientRx<B> = ::client::dispatch::Receiver<Request<B>, Response<Body>>;
/// An mpsc channel is used to help notify the `Connection` task when *all*
/// other handles to it have been dropped, so that it can shutdown.
type ConnDropRef = mpsc::Sender<Never>;

/// A oneshot channel watches the `Connection` task, and when it completes,
/// the "dispatch" task will be notified and can shutdown sooner.
type ConnEof = oneshot::Receiver<Never>;

pub(crate) struct Client<T, B>
where
    B: Payload,
{
    executor: Exec,
    rx: ClientRx<B>,
    state: State<T, SendBuf<B::Data>>,
}

enum State<T, B> where B: IntoBuf {
    Handshaking(Handshake<T, B>),
    Ready(SendRequest<B>, ConnDropRef, ConnEof),
}

impl<T, B> Client<T, B>
where
    T: AsyncRead + AsyncWrite + Send + 'static,
    B: Payload,
{
    pub(crate) fn new(io: T, rx: ClientRx<B>, builder: &Builder, exec: Exec) -> Client<T, B> {
        let handshake = builder.handshake(io);

        Client {
            executor: exec,
            rx: rx,
            state: State::Handshaking(handshake),
        }
    }
}

impl<T, B> Future for Client<T, B>
where
    T: AsyncRead + AsyncWrite + Send + 'static,
    B: Payload + 'static,
{
    type Item = Dispatched;
    type Error = ::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        loop {
            let next = match self.state {
                State::Handshaking(ref mut h) => {
                    let (request_tx, conn) = try_ready!(h.poll().map_err(::Error::new_h2));
                    // An mpsc channel is used entirely to detect when the
                    // 'Client' has been dropped. This is to get around a bug
                    // in h2 where dropping all SendRequests won't notify a
                    // parked Connection.
                    let (tx, rx) = mpsc::channel(0);
                    let (cancel_tx, cancel_rx) = oneshot::channel();
                    let rx = rx.into_future()
                        .map(|(msg, _)| match msg {
                            Some(never) => match never {},
                            None => (),
                        })
                        .map_err(|_| -> Never { unreachable!("mpsc cannot error") });
                    let fut = conn
                        .inspect(move |_| {
                            drop(cancel_tx);
                            trace!("connection complete")
                        })
                        .map_err(|e| debug!("connection error: {}", e))
                        .select2(rx)
                        .then(|res| match res {
                            Ok(Either::A(((), _))) |
                            Err(Either::A(((), _))) => {
                                // conn has finished either way
                                Either::A(future::ok(()))
                            },
                            Ok(Either::B(((), conn))) => {
                                // mpsc has been dropped, hopefully polling
                                // the connection some more should start shutdown
                                // and then close
                                trace!("send_request dropped, starting conn shutdown");
                                Either::B(conn)
                            }
                            Err(Either::B((never, _))) => match never {},
                        });
                    self.executor.execute(fut)?;
                    State::Ready(request_tx, tx, cancel_rx)
                },
                State::Ready(ref mut tx, ref conn_dropper, ref mut cancel_rx) => {
                    match tx.poll_ready() {
                        Ok(Async::Ready(())) => (),
                        Ok(Async::NotReady) => return Ok(Async::NotReady),
                        Err(err) => {
                            return if err.reason() == Some(::h2::Reason::NO_ERROR) {
                                trace!("connection gracefully shutdown");
                                Ok(Async::Ready(Dispatched::Shutdown))
                            } else {
                                Err(::Error::new_h2(err))
                            };
                        }
                    }
                    match self.rx.poll() {
                        Ok(Async::Ready(Some((req, cb)))) => {
                            // check that future hasn't been canceled already
                            if cb.is_canceled() {
                                trace!("request callback is canceled");
                                continue;
                            }
                            let (head, body) = req.into_parts();
                            let mut req = ::http::Request::from_parts(head, ());
                            super::strip_connection_headers(req.headers_mut(), true);
                            if let Some(len) = body.content_length() {
                                headers::set_content_length_if_missing(req.headers_mut(), len);
                            }
                            let eos = body.is_end_stream();
                            let (fut, body_tx) = match tx.send_request(req, eos) {
                                Ok(ok) => ok,
                                Err(err) => {
                                    debug!("client send request error: {}", err);
                                    cb.send(Err((::Error::new_h2(err), None)));
                                    continue;
                                }
                            };
                            if !eos {
                                let mut pipe = PipeToSendStream::new(body, body_tx)
                                    .map_err(|e| debug!("client request body error: {}", e));

                                // eagerly see if the body pipe is ready and
                                // can thus skip allocating in the executor
                                match pipe.poll() {
                                    Ok(Async::Ready(())) | Err(()) => (),
                                    Ok(Async::NotReady) => {
                                        let conn_drop_ref = conn_dropper.clone();
                                        let pipe = pipe.then(move |x| {
                                                drop(conn_drop_ref);
                                                x
                                            });
                                        self.executor.execute(pipe)?;
                                    }
                                }
                            }

                            let fut = fut
                                .then(move |result| {
                                    match result {
                                        Ok(res) => {
                                            let content_length = content_length_parse_all(res.headers());
                                            let res = res.map(|stream|
                                                ::Body::h2(stream, content_length));
                                            Ok(res)
                                        },
                                        Err(err) => {
                                            debug!("client response error: {}", err);
                                            Err((::Error::new_h2(err), None))
                                        }
                                    }
                                });
                            self.executor.execute(cb.send_when(fut))?;
                            continue;
                        },

                        Ok(Async::NotReady) => {
                            match cancel_rx.poll() {
                                Ok(Async::Ready(never)) => match never {},
                                Ok(Async::NotReady) => return Ok(Async::NotReady),
                                Err(_conn_is_eof) => {
                                    trace!("connection task is closed, closing dispatch task");
                                    return Ok(Async::Ready(Dispatched::Shutdown));
                                }
                            }
                        },

                        Ok(Async::Ready(None)) => {
                            trace!("client::dispatch::Sender dropped");
                            return Ok(Async::Ready(Dispatched::Shutdown));
                        },
                        Err(never) => match never {},
                    }
                },
            };
            self.state = next;
        }
    }
}
