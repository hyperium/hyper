use bytes::IntoBuf;
use futures::{Async, Future, Poll, Stream};
use h2::client::{Builder, Handshake, SendRequest};
use tokio_io::{AsyncRead, AsyncWrite};

use headers::content_length_parse_all;
use body::Payload;
use ::common::Exec;
use headers;
use ::proto::Dispatched;
use super::{PipeToSendStream, SendBuf};
use ::{Body, Request, Response};

type ClientRx<B> = ::client::dispatch::Receiver<Request<B>, Response<Body>>;

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
    Ready(SendRequest<B>),
}

impl<T, B> Client<T, B>
where
    T: AsyncRead + AsyncWrite + Send + 'static,
    B: Payload,
{
    pub(crate) fn new(io: T, rx: ClientRx<B>, exec: Exec) -> Client<T, B> {
        let handshake = Builder::new()
            // we don't expose PUSH promises yet
            .enable_push(false)
            .handshake(io);

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
                    let fut = conn
                        .inspect(|_| trace!("connection complete"))
                        .map_err(|e| debug!("connection error: {}", e));
                    self.executor.execute(fut)?;
                    State::Ready(request_tx)
                },
                State::Ready(ref mut tx) => {
                    try_ready!(tx.poll_ready().map_err(::Error::new_h2));
                    match self.rx.poll() {
                        Ok(Async::Ready(Some((req, mut cb)))) => {
                            // check that future hasn't been canceled already
                            if let Async::Ready(()) = cb.poll_cancel().expect("poll_cancel cannot error") {
                                trace!("request canceled");
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
                                    let _ = cb.send(Err((::Error::new_h2(err), None)));
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
                                            let _ = cb.send(Ok(res));
                                        },
                                        Err(err) => {
                                            debug!("client response error: {}", err);
                                            let _ = cb.send(Err((::Error::new_h2(err), None)));
                                        }
                                    }
                                    Ok(())
                                });
                            self.executor.execute(fut)?;
                            continue;
                        },

                        Ok(Async::NotReady) => return Ok(Async::NotReady),

                        Ok(Async::Ready(None)) |
                        Err(_) => {
                            trace!("client::dispatch::Sender dropped");
                            return Ok(Async::Ready(Dispatched::Shutdown));
                        }
                    }
                },
            };
            self.state = next;
        }
    }
}
