use bytes::IntoBuf;
use futures::{Async, Future, Poll, Stream};
use h2::client::{Builder, Handshake, SendRequest};
use tokio_io::{AsyncRead, AsyncWrite};

use ::client::Exec;
use ::client::dispatch::Receiver;
use super::{PipeToSendStream, SendBuf};

pub struct Client<T, B>
where
    B: Stream,
    B::Item: AsRef<[u8]> + 'static,
{
    executor: Exec,
    rx: Receiver<::Request<B>, ::Response>,
    state: State<T, SendBuf<B::Item>>,
}

enum State<T, B> where B: IntoBuf {
    Handshaking(Handshake<T, B>),
    Ready(SendRequest<B>),
}

impl<T, B> Client<T, B> 
where
    T: AsyncRead + AsyncWrite + 'static,
    B: Stream,
    B::Item: AsRef<[u8]> + 'static,
{
    pub(crate) fn new(io: T, rx: Receiver<::Request<B>, ::Response>, exec: Exec) -> Client<T, B> {
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
    T: AsyncRead + AsyncWrite + 'static,
    B: Stream<Error=::Error> + 'static,
    B::Item: AsRef<[u8]> + 'static,
{
    type Item = ();
    type Error = ::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        loop {
            let next = match self.state {
                State::Handshaking(ref mut h) => {
                    let (request_tx, conn) = try_ready!(h.poll().map_err(::Error::from_h2));
                    self.executor.execute(conn.map_err(|e| debug!("client h2 connection error: {}", e)))?;
                    State::Ready(request_tx)
                },
                State::Ready(ref mut tx) => {
                    try_ready!(tx.poll_ready().map_err(::Error::from_h2));
                    match self.rx.poll() {
                        Ok(Async::Ready(Some((req, mut cb)))) => {
                            // check that future hasn't been canceled already
                            if let Async::Ready(()) = cb.poll_cancel().expect("poll_cancel cannot error") {
                                trace!("request canceled");
                                continue;
                            }
                            let (head, body) = req.into_http().into_parts();
                            let mut req = ::http::Request::from_parts(head, ());
                            super::strip_connection_headers(req.headers_mut());
                            let (fut, body_tx) = match tx.send_request(req, body.is_none()) {
                                Ok(ok) => ok,
                                Err(err) => {
                                    debug!("client send request error: {}", err);
                                    let _ = cb.send(Err(::Error::from_h2(err)));
                                    continue;
                                }
                            };
                            if let Some(body) = body {
                                let pipe = PipeToSendStream::new(body, body_tx);
                                self.executor.execute(pipe.map_err(|e| debug!("client request body error: {}", e)))?;
                            }

                            let fut = fut
                                .then(move |result| {
                                    match result {
                                        Ok(res) => {
                                            let res = res.map(::Body::h2);
                                            let _ = cb.send(Ok(res.into()));
                                        },
                                        Err(err) => {
                                            debug!("client response error: {}", err);
                                            let _ = cb.send(Err(::Error::from_h2(err)));
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
                            trace!("client tx dropped");
                            return Ok(Async::Ready(()));
                        }
                    }
                },
            };
            self.state = next;
        }
    }
}
