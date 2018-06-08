use futures::{Async, Future, Poll, Stream};
use h2::Reason;
use h2::server::{Builder, Connection, Handshake, SendResponse};
use tokio_io::{AsyncRead, AsyncWrite};

use ::body::Payload;
use ::common::Exec;
use ::headers;
use ::service::Service;
use super::{PipeToSendStream, SendBuf};

use ::{Body, Response};

pub(crate) struct Server<T, S, B>
where
    S: Service,
    B: Payload,
{
    exec: Exec,
    service: S,
    state: State<T, B>,
}

enum State<T, B>
where
    B: Payload,
{
    Handshaking(Handshake<T, SendBuf<B::Data>>),
    Serving(Serving<T, B>),
    Closed,
}

struct Serving<T, B>
where
    B: Payload,
{
    conn: Connection<T, SendBuf<B::Data>>,
}


impl<T, S, B> Server<T, S, B>
where
    T: AsyncRead + AsyncWrite,
    S: Service<ReqBody=Body, ResBody=B>,
    S::Error: Into<Box<::std::error::Error + Send + Sync>>,
    S::Future: Send + 'static,
    B: Payload,
{
    pub(crate) fn new(io: T, service: S, exec: Exec) -> Server<T, S, B> {
        let handshake = Builder::new()
            .handshake(io);
        Server {
            exec,
            state: State::Handshaking(handshake),
            service,
        }
    }

    pub fn graceful_shutdown(&mut self) {
        trace!("graceful_shutdown");
        match self.state {
            State::Handshaking(..) => {
                // fall-through, to replace state with Closed
            },
            State::Serving(ref mut srv) => {
                srv.conn.graceful_shutdown();
                return;
            },
            State::Closed => {
                return;
            }
        }
        self.state = State::Closed;
    }
}

impl<T, S, B> Future for Server<T, S, B>
where
    T: AsyncRead + AsyncWrite,
    S: Service<ReqBody=Body, ResBody=B>,
    S::Error: Into<Box<::std::error::Error + Send + Sync>>,
    S::Future: Send + 'static,
    B: Payload,
{
    type Item = ();
    type Error = ::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        loop {
            let next = match self.state {
                State::Handshaking(ref mut h) => {
                    let conn = try_ready!(h.poll().map_err(::Error::new_h2));
                    State::Serving(Serving {
                        conn: conn,
                    })
                },
                State::Serving(ref mut srv) => {
                    return srv.poll_server(&mut self.service, &self.exec);
                }
                State::Closed => {
                    // graceful_shutdown was called before handshaking finished,
                    // nothing to do here...
                    return Ok(Async::Ready(()));
                }
            };
            self.state = next;
        }
    }
}

impl<T, B> Serving<T, B>
where
    T: AsyncRead + AsyncWrite,
    B: Payload,
{
    fn poll_server<S>(&mut self, service: &mut S, exec: &Exec) -> Poll<(), ::Error>
    where
        S: Service<
            ReqBody=Body,
            ResBody=B,
        >,
        S::Error: Into<Box<::std::error::Error + Send + Sync>>,
        S::Future: Send + 'static,
    {
        while let Some((req, respond)) = try_ready!(self.conn.poll().map_err(::Error::new_h2)) {
            trace!("incoming request");
            let req = req.map(::Body::h2);
            let fut = H2Stream::new(service.call(req), respond);
            exec.execute(fut);
        }

        // no more incoming streams...
        trace!("incoming connection complete");
        Ok(Async::Ready(()))
    }
}

struct H2Stream<F, B>
where
    B: Payload,
{
    reply: SendResponse<SendBuf<B::Data>>,
    state: H2StreamState<F, B>,
}

enum H2StreamState<F, B>
where
    B: Payload,
{
    Service(F),
    Body(PipeToSendStream<B>),
}

impl<F, B> H2Stream<F, B>
where
    F: Future<Item=Response<B>>,
    F::Error: Into<Box<::std::error::Error + Send + Sync>>,
    B: Payload,
{
    fn new(fut: F, respond: SendResponse<SendBuf<B::Data>>) -> H2Stream<F, B> {
        H2Stream {
            reply: respond,
            state: H2StreamState::Service(fut),
        }
    }

    fn poll2(&mut self) -> Poll<(), ::Error> {
        loop {
            let next = match self.state {
                H2StreamState::Service(ref mut h) => {
                    let res = try_ready!(h.poll().map_err(::Error::new_user_service));
                    let (head, body) = res.into_parts();
                    let mut res = ::http::Response::from_parts(head, ());
                    super::strip_connection_headers(res.headers_mut());
                    if let Some(len) = body.content_length() {
                        headers::set_content_length_if_missing(res.headers_mut(), len);
                    }
                    macro_rules! reply {
                        ($eos:expr) => ({
                            match self.reply.send_response(res, $eos) {
                                Ok(tx) => tx,
                                Err(e) => {
                                    trace!("send response error: {}", e);
                                    self.reply.send_reset(Reason::INTERNAL_ERROR);
                                    return Err(::Error::new_h2(e));
                                }
                            }
                        })
                    }
                    if !body.is_end_stream() {
                        let body_tx = reply!(false);
                        H2StreamState::Body(PipeToSendStream::new(body, body_tx))
                    } else {
                        reply!(true);
                        return Ok(Async::Ready(()));
                    }
                },
                H2StreamState::Body(ref mut pipe) => {
                    return pipe.poll();
                }
            };
            self.state = next;
        }
    }
}

impl<F, B> Future for H2Stream<F, B>
where
    F: Future<Item=Response<B>>,
    F::Error: Into<Box<::std::error::Error + Send + Sync>>,
    B: Payload,
{
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        self.poll2()
            .map_err(|e| debug!("stream error: {}", e))
    }
}

