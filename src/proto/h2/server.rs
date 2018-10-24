use futures::{Async, Future, Poll, Stream};
use h2::Reason;
use h2::server::{Builder, Connection, Handshake, SendResponse};
use tokio_io::{AsyncRead, AsyncWrite};

use ::headers::content_length_parse_all;
use ::body::Payload;
use body::internal::FullDataArg;
use ::common::exec::H2Exec;
use ::headers;
use ::service::Service;
use ::proto::Dispatched;
use super::{PipeToSendStream, SendBuf};

use ::{Body, Response};

pub(crate) struct Server<T, S, B, E>
where
    S: Service,
    B: Payload,
{
    exec: E,
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


impl<T, S, B, E> Server<T, S, B, E>
where
    T: AsyncRead + AsyncWrite,
    S: Service<ReqBody=Body, ResBody=B>,
    S::Error: Into<Box<::std::error::Error + Send + Sync>>,
    //S::Future: Send + 'static,
    B: Payload,
    E: H2Exec<S::Future, B>,
{
    pub(crate) fn new(io: T, service: S, exec: E) -> Server<T, S, B, E> {
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

impl<T, S, B, E> Future for Server<T, S, B, E>
where
    T: AsyncRead + AsyncWrite,
    S: Service<ReqBody=Body, ResBody=B>,
    S::Error: Into<Box<::std::error::Error + Send + Sync>>,
    //S::Future: Send + 'static,
    B: Payload,
    E: H2Exec<S::Future, B>,
{
    type Item = Dispatched;
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
                    try_ready!(srv.poll_server(&mut self.service, &self.exec));
                    return Ok(Async::Ready(Dispatched::Shutdown));
                }
                State::Closed => {
                    // graceful_shutdown was called before handshaking finished,
                    // nothing to do here...
                    return Ok(Async::Ready(Dispatched::Shutdown));
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
    fn poll_server<S, E>(&mut self, service: &mut S, exec: &E) -> Poll<(), ::Error>
    where
        S: Service<
            ReqBody=Body,
            ResBody=B,
        >,
        S::Error: Into<Box<::std::error::Error + Send + Sync>>,
        E: H2Exec<S::Future, B>,
    {
        while let Some((req, respond)) = try_ready!(self.conn.poll().map_err(::Error::new_h2)) {
            trace!("incoming request");
            let content_length = content_length_parse_all(req.headers());
            let req = req.map(|stream| {
                ::Body::h2(stream, content_length)
            });
            let mut fut = H2Stream::new(service.call(req), respond);

            // try to eagerly poll the future, so that we might
            // not need to allocate a new task...
            match fut.poll() {
                Ok(Async::Ready(())) | Err(()) => (),
                Ok(Async::NotReady) => {
                    exec.execute_h2stream(fut)?;
                }
            }
        }

        // no more incoming streams...
        trace!("incoming connection complete");
        Ok(Async::Ready(()))
    }
}

#[allow(missing_debug_implementations)]
pub struct H2Stream<F, B>
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
                    let res = match h.poll() {
                        Ok(Async::Ready(r)) => r,
                        Ok(Async::NotReady) => {
                            // Body is not yet ready, so we want to check if the client has sent a
                            // RST_STREAM frame which would cancel the current request.
                            if let Async::Ready(reason) =
                                self.reply.poll_reset().map_err(|e| ::Error::new_h2(e))?
                            {
                                debug!("stream received RST_STREAM: {:?}", reason);
                                return Err(::Error::new_h2(reason.into()));
                            }
                            return Ok(Async::NotReady);
                        }
                        Err(e) => return Err(::Error::new_user_service(e)),
                    };

                    let (head, mut body) = res.into_parts();
                    let mut res = ::http::Response::from_parts(head, ());
                    super::strip_connection_headers(res.headers_mut(), false);

                    // set Date header if it isn't already set...
                    res
                        .headers_mut()
                        .entry(::http::header::DATE)
                        .expect("DATE is a valid HeaderName")
                        .or_insert_with(::proto::h1::date::update_and_header_value);

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

                    if let Some(full) = body.__hyper_full_data(FullDataArg(())).0 {
                        let mut body_tx = reply!(false);
                        let buf = SendBuf(Some(full));
                        body_tx
                            .send_data(buf, true)
                            .map_err(::Error::new_body_write)?;
                        return Ok(Async::Ready(()));
                    }

                    // automatically set Content-Length from body...
                    if let Some(len) = body.content_length() {
                        headers::set_content_length_if_missing(res.headers_mut(), len);
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

