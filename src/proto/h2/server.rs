use std::error::Error as StdError;
use std::marker::Unpin;

use futures_core::Stream;
use h2::Reason;
use h2::server::{Builder, Connection, Handshake, SendResponse};
use tokio_io::{AsyncRead, AsyncWrite};

use crate::body::Payload;
use crate::body::internal::FullDataArg;
use crate::common::exec::H2Exec;
use crate::common::{Future, Pin, Poll, task};
use crate::headers;
use crate::headers::content_length_parse_all;
use crate::service::Service;
use crate::proto::Dispatched;
use super::{PipeToSendStream, SendBuf};

use crate::{Body, Response};

pub(crate) struct Server<T, S, B, E>
where
    S: Service<Body>,
    B: Payload,
{
    exec: E,
    service: S,
    state: State<T, B>,
}

// TODO: fix me
impl<T, S: Service<Body>, B: Payload, E> Unpin for Server<T, S, B, E> {}

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
    closing: Option<crate::Error>,
}


impl<T, S, B, E> Server<T, S, B, E>
where
    T: AsyncRead + AsyncWrite + Unpin,
    S: Service<Body, ResBody=B>,
    S::Error: Into<Box<dyn StdError + Send + Sync>>,
    B: Payload,
    B::Data: Unpin,
    E: H2Exec<S::Future, B>,
{
    pub(crate) fn new(io: T, service: S, builder: &Builder, exec: E) -> Server<T, S, B, E> {
        let handshake = builder.handshake(io);
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
                if srv.closing.is_none() {
                    srv.conn.graceful_shutdown();
                }
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
    T: AsyncRead + AsyncWrite + Unpin,
    S: Service<Body, ResBody=B>,
    S::Error: Into<Box<dyn StdError + Send + Sync>>,
    B: Payload,
    B::Data: Unpin,
    E: H2Exec<S::Future, B>,
{
    type Output = crate::Result<Dispatched>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Self::Output> {
        let me = &mut *self;
        loop {
            let next = match me.state {
                State::Handshaking(ref mut h) => {
                    let conn = ready!(Pin::new(h).poll(cx).map_err(crate::Error::new_h2))?;
                    State::Serving(Serving {
                        conn,
                        closing: None,
                    })
                },
                State::Serving(ref mut srv) => {
                    ready!(srv.poll_server(cx, &mut me.service, &mut me.exec))?;
                    return Poll::Ready(Ok(Dispatched::Shutdown));
                }
                State::Closed => {
                    // graceful_shutdown was called before handshaking finished,
                    // nothing to do here...
                    return Poll::Ready(Ok(Dispatched::Shutdown));
                }
            };
            me.state = next;
        }
    }
}

impl<T, B> Serving<T, B>
where
    T: AsyncRead + AsyncWrite + Unpin,
    B: Payload,
    B::Data: Unpin,
{
    fn poll_server<S, E>(&mut self, cx: &mut task::Context<'_>, service: &mut S, exec: &mut E) -> Poll<crate::Result<()>>
    where
        S: Service<
            Body,
            ResBody=B,
        >,
        S::Error: Into<Box<dyn StdError + Send + Sync>>,
        E: H2Exec<S::Future, B>,
    {
        if self.closing.is_none() {
            loop {
                // At first, polls the readiness of supplied service.
                match service.poll_ready(cx) {
                    Poll::Ready(Ok(())) => (),
                    Poll::Pending => {
                        // use `poll_close` instead of `poll`, in order to avoid accepting a request.
                        ready!(self.conn.poll_close(cx).map_err(crate::Error::new_h2))?;
                        trace!("incoming connection complete");
                        return Poll::Ready(Ok(()));
                    }
                    Poll::Ready(Err(err)) => {
                        let err = crate::Error::new_user_service(err);
                        debug!("service closed: {}", err);

                        let reason = err.h2_reason();
                        if reason == Reason::NO_ERROR {
                            // NO_ERROR is only used for graceful shutdowns...
                            trace!("interpretting NO_ERROR user error as graceful_shutdown");
                            self.conn.graceful_shutdown();
                        } else {
                            trace!("abruptly shutting down with {:?}", reason);
                            self.conn.abrupt_shutdown(reason);
                        }
                        self.closing = Some(err);
                        break;
                    }
                }

                // When the service is ready, accepts an incoming request.
                match ready!(self.conn.poll_accept(cx)) {
                    Some(Ok((req, respond))) => {
                        trace!("incoming request");
                        let content_length = content_length_parse_all(req.headers());
                        let req = req.map(|stream| {
                            crate::Body::h2(stream, content_length)
                        });
                        let fut = H2Stream::new(service.call(req), respond);
                        exec.execute_h2stream(fut)?;
                    },
                    Some(Err(e)) => {
                        return Poll::Ready(Err(crate::Error::new_h2(e)));
                    },
                    None => {
                        // no more incoming streams...
                        trace!("incoming connection complete");
                        return Poll::Ready(Ok(()));
                    },
                }
            }
        }

        debug_assert!(self.closing.is_some(), "poll_server broke loop without closing");

        ready!(self.conn.poll_close(cx).map_err(crate::Error::new_h2))?;

        Poll::Ready(Err(self.closing.take().expect("polled after error")))
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
    //F: Future<Item=Response<B>>,
    //F::Error: Into<Box<dyn StdError + Send + Sync>>,
    B: Payload,
{
    fn new(fut: F, respond: SendResponse<SendBuf<B::Data>>) -> H2Stream<F, B> {
        H2Stream {
            reply: respond,
            state: H2StreamState::Service(fut),
        }
    }
}

impl<F, B, E> H2Stream<F, B>
where
    F: Future<Output = Result<Response<B>, E>>,
    B: Payload + Unpin,
    B::Data: Unpin,
    E: Into<Box<dyn StdError + Send + Sync>>,
{
    fn poll2(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<crate::Result<()>> {
        // Safety: State::{Service, Body} futures are never moved
        let me = unsafe { self.get_unchecked_mut() };
        loop {
            let next = match me.state {
                H2StreamState::Service(ref mut h) => {
                    let res = match unsafe { Pin::new_unchecked(h) }.poll(cx) {
                        Poll::Ready(Ok(r)) => r,
                        Poll::Pending => {
                            // Response is not yet ready, so we want to check if the client has sent a
                            // RST_STREAM frame which would cancel the current request.
                            if let Poll::Ready(reason) =
                                me.reply.poll_reset(cx).map_err(|e| crate::Error::new_h2(e))?
                            {
                                debug!("stream received RST_STREAM: {:?}", reason);
                                return Poll::Ready(Err(crate::Error::new_h2(reason.into())));
                            }
                            return Poll::Pending;
                        }
                        Poll::Ready(Err(e)) => {
                            let err = crate::Error::new_user_service(e);
                            warn!("http2 service errored: {}", err);
                            me.reply.send_reset(err.h2_reason());
                            return Poll::Ready(Err(err));
                        },
                    };

                    let (head, mut body) = res.into_parts();
                    let mut res = ::http::Response::from_parts(head, ());
                    super::strip_connection_headers(res.headers_mut(), false);

                    // set Date header if it isn't already set...
                    res
                        .headers_mut()
                        .entry(::http::header::DATE)
                        .expect("DATE is a valid HeaderName")
                        .or_insert_with(crate::proto::h1::date::update_and_header_value);

                    macro_rules! reply {
                        ($eos:expr) => ({
                            match me.reply.send_response(res, $eos) {
                                Ok(tx) => tx,
                                Err(e) => {
                                    debug!("send response error: {}", e);
                                    me.reply.send_reset(Reason::INTERNAL_ERROR);
                                    return Poll::Ready(Err(crate::Error::new_h2(e)));
                                }
                            }
                        })
                    }

                    // automatically set Content-Length from body...
                    if let Some(len) = body.content_length() {
                        headers::set_content_length_if_missing(res.headers_mut(), len);
                    }

                    if let Some(full) = body.__hyper_full_data(FullDataArg(())).0 {
                        let mut body_tx = reply!(false);
                        let buf = SendBuf(Some(full));
                        body_tx
                            .send_data(buf, true)
                            .map_err(crate::Error::new_body_write)?;
                        return Poll::Ready(Ok(()));
                    }

                    if !body.is_end_stream() {
                        let body_tx = reply!(false);
                        H2StreamState::Body(PipeToSendStream::new(body, body_tx))
                    } else {
                        reply!(true);
                        return Poll::Ready(Ok(()));
                    }
                },
                H2StreamState::Body(ref mut pipe) => {
                    return Pin::new(pipe).poll(cx);
                }
            };
            me.state = next;
        }
    }
}

impl<F, B, E> Future for H2Stream<F, B>
where
    F: Future<Output = Result<Response<B>, E>>,
    B: Payload + Unpin,
    B::Data: Unpin,
    E: Into<Box<dyn StdError + Send + Sync>>,
{
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Self::Output> {
        self.poll2(cx).map(|res| {
            if let Err(e) = res {
                debug!("stream error: {}", e);
            }
        })
    }
}
