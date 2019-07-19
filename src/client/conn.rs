//! Lower-level client connection API.
//!
//! The types in this module are to provide a lower-level API based around a
//! single connection. Connecting to a host, pooling connections, and the like
//! are not handled at this level. This module provides the building blocks to
//! customize those things externally.
//!
//! If don't have need to manage connections yourself, consider using the
//! higher-level [Client](super) API.
use std::fmt;
use std::marker::PhantomData;
use std::mem;
use std::sync::Arc;

use bytes::Bytes;
use futures_util::future::{self, Either, FutureExt as _};
use h2;
use tokio_io::{AsyncRead, AsyncWrite};

use crate::body::Payload;
use crate::common::{Exec, Future, Pin, Poll, task};
use crate::upgrade::Upgraded;
use crate::proto;
use super::dispatch;
use crate::{Body, Request, Response};

type Http1Dispatcher<T, B, R> = proto::dispatch::Dispatcher<
    proto::dispatch::Client<B>,
    B,
    T,
    R,
>;
type ConnEither<T, B> = Either<
    Http1Dispatcher<T, B, proto::h1::ClientTransaction>,
    proto::h2::Client<T, B>,
>;

/// Returns a `Handshake` future over some IO.
///
/// This is a shortcut for `Builder::new().handshake(io)`.
pub fn handshake<T>(io: T) -> Handshake<T, crate::Body>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    Builder::new()
        .handshake(io)
}

/// The sender side of an established connection.
pub struct SendRequest<B> {
    dispatch: dispatch::Sender<Request<B>, Response<Body>>,
}


/// A future that processes all HTTP state for the IO object.
///
/// In most cases, this should just be spawned into an executor, so that it
/// can process incoming and outgoing messages, notice hangups, and the like.
#[must_use = "futures do nothing unless polled"]
pub struct Connection<T, B>
where
    T: AsyncRead + AsyncWrite + Send + 'static,
    B: Payload + 'static,
{
    inner: Option<ConnEither<T, B>>,
}


/// A builder to configure an HTTP connection.
///
/// After setting options, the builder is used to create a `Handshake` future.
#[derive(Clone, Debug)]
pub struct Builder {
    pub(super) exec: Exec,
    h1_writev: bool,
    h1_title_case_headers: bool,
    h1_read_buf_exact_size: Option<usize>,
    h1_max_buf_size: Option<usize>,
    http2: bool,
    h2_builder: h2::client::Builder,
}

/// A future setting up HTTP over an IO object.
///
/// If successful, yields a `(SendRequest, Connection)` pair.
#[must_use = "futures do nothing unless polled"]
pub struct Handshake<T, B> {
    builder: Builder,
    io: Option<T>,
    _marker: PhantomData<fn(B)>,
}

/// A future returned by `SendRequest::send_request`.
///
/// Yields a `Response` if successful.
#[must_use = "futures do nothing unless polled"]
pub struct ResponseFuture {
    inner: ResponseFutureState
}

enum ResponseFutureState {
    Waiting(dispatch::Promise<Response<Body>>),
    // Option is to be able to `take()` it in `poll`
    Error(Option<crate::Error>),
}

/// Deconstructed parts of a `Connection`.
///
/// This allows taking apart a `Connection` at a later time, in order to
/// reclaim the IO object, and additional related pieces.
#[derive(Debug)]
pub struct Parts<T> {
    /// The original IO object used in the handshake.
    pub io: T,
    /// A buffer of bytes that have been read but not processed as HTTP.
    ///
    /// For instance, if the `Connection` is used for an HTTP upgrade request,
    /// it is possible the server sent back the first bytes of the new protocol
    /// along with the response upgrade.
    ///
    /// You will want to check for any existing bytes if you plan to continue
    /// communicating on the IO object.
    pub read_buf: Bytes,
    _inner: (),
}

// ========== internal client api

// A `SendRequest` that can be cloned to send HTTP2 requests.
// private for now, probably not a great idea of a type...
#[must_use = "futures do nothing unless polled"]
pub(super) struct Http2SendRequest<B> {
    dispatch: dispatch::UnboundedSender<Request<B>, Response<Body>>,
}

// ===== impl SendRequest

impl<B> SendRequest<B>
{
    /// Polls to determine whether this sender can be used yet for a request.
    ///
    /// If the associated connection is closed, this returns an Error.
    pub fn poll_ready(&mut self, cx: &mut task::Context<'_>) -> Poll<crate::Result<()>> {
        self.dispatch.poll_ready(cx)
    }

    pub(super) fn when_ready(self) -> impl Future<Output=crate::Result<Self>> {
        let mut me = Some(self);
        future::poll_fn(move |cx| {
            ready!(me.as_mut().unwrap().poll_ready(cx))?;
            Poll::Ready(Ok(me.take().unwrap()))
        })
    }

    pub(super) fn is_ready(&self) -> bool {
        self.dispatch.is_ready()
    }

    pub(super) fn is_closed(&self) -> bool {
        self.dispatch.is_closed()
    }

    pub(super) fn into_http2(self) -> Http2SendRequest<B> {
        Http2SendRequest {
            dispatch: self.dispatch.unbound(),
        }
    }
}

impl<B> SendRequest<B>
where
    B: Payload + 'static,
{
    /// Sends a `Request` on the associated connection.
    ///
    /// Returns a future that if successful, yields the `Response`.
    ///
    /// # Note
    ///
    /// There are some key differences in what automatic things the `Client`
    /// does for you that will not be done here:
    ///
    /// - `Client` requires absolute-form `Uri`s, since the scheme and
    ///   authority are needed to connect. They aren't required here.
    /// - Since the `Client` requires absolute-form `Uri`s, it can add
    ///   the `Host` header based on it. You must add a `Host` header yourself
    ///   before calling this method.
    /// - Since absolute-form `Uri`s are not required, if received, they will
    ///   be serialized as-is.
    ///
    /// # Example
    ///
    /// ```
    /// # #![feature(async_await)]
    /// # use http::header::HOST;
    /// # use hyper::client::conn::SendRequest;
    /// # use hyper::Body;
    /// use hyper::Request;
    ///
    /// # async fn doc(mut tx: SendRequest<Body>) -> hyper::Result<()> {
    /// // build a Request
    /// let req = Request::builder()
    ///     .uri("/foo/bar")
    ///     .header(HOST, "hyper.rs")
    ///     .body(Body::empty())
    ///     .unwrap();
    ///
    /// // send it and await a Response
    /// let res = tx.send_request(req).await?;
    /// // assert the Response
    /// assert!(res.status().is_success());
    /// # Ok(())
    /// # }
    /// # fn main() {}
    /// ```
    pub fn send_request(&mut self, req: Request<B>) -> ResponseFuture {
        let inner = match self.dispatch.send(req) {
            Ok(rx) => {
                ResponseFutureState::Waiting(rx)
            },
            Err(_req) => {
                debug!("connection was not ready");
                let err = crate::Error::new_canceled().with("connection was not ready");
                ResponseFutureState::Error(Some(err))
            }
        };

        ResponseFuture {
            inner,
        }
    }

    pub(crate) fn send_request_retryable(&mut self, req: Request<B>) -> impl Future<Output = Result<Response<Body>, (crate::Error, Option<Request<B>>)>> + Unpin
    where
        B: Send,
    {
        match self.dispatch.try_send(req) {
            Ok(rx) => {
                Either::Left(rx.then(move |res| {
                    match res {
                        Ok(Ok(res)) => future::ok(res),
                        Ok(Err(err)) => future::err(err),
                        // this is definite bug if it happens, but it shouldn't happen!
                        Err(_) => panic!("dispatch dropped without returning error"),
                    }
                }))
            },
            Err(req) => {
                debug!("connection was not ready");
                let err = crate::Error::new_canceled().with("connection was not ready");
                Either::Right(future::err((err, Some(req))))
            }
        }
    }
}

/* TODO(0.12.0): when we change from tokio-service to tower.
impl<T, B> Service for SendRequest<T, B> {
    type Request = Request<B>;
    type Response = Response;
    type Error = ::Error;
    type Future = ResponseFuture;

    fn call(&self, req: Self::Request) -> Self::Future {

    }
}
*/

impl<B> fmt::Debug for SendRequest<B> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("SendRequest")
            .finish()
    }
}

// ===== impl Http2SendRequest

impl<B> Http2SendRequest<B> {
    pub(super) fn is_ready(&self) -> bool {
        self.dispatch.is_ready()
    }

    pub(super) fn is_closed(&self) -> bool {
        self.dispatch.is_closed()
    }
}

impl<B> Http2SendRequest<B>
where
    B: Payload + 'static,
{
    pub(super) fn send_request_retryable(&mut self, req: Request<B>) -> impl Future<Output=Result<Response<Body>, (crate::Error, Option<Request<B>>)>>
    where
        B: Send,
    {
        match self.dispatch.try_send(req) {
            Ok(rx) => {
                Either::Left(rx.then(move |res| {
                    match res {
                        Ok(Ok(res)) => future::ok(res),
                        Ok(Err(err)) => future::err(err),
                        // this is definite bug if it happens, but it shouldn't happen!
                        Err(_) => panic!("dispatch dropped without returning error"),
                    }
                }))
            },
            Err(req) => {
                debug!("connection was not ready");
                let err = crate::Error::new_canceled().with("connection was not ready");
                Either::Right(future::err((err, Some(req))))
            }
        }
    }
}

impl<B> fmt::Debug for Http2SendRequest<B> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Http2SendRequest")
            .finish()
    }
}

impl<B> Clone for Http2SendRequest<B> {
    fn clone(&self) -> Self {
        Http2SendRequest {
            dispatch: self.dispatch.clone(),
        }
    }
}

// ===== impl Connection

impl<T, B> Connection<T, B>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    B: Payload + 'static,
{
    /// Return the inner IO object, and additional information.
    ///
    /// Only works for HTTP/1 connections. HTTP/2 connections will panic.
    pub fn into_parts(self) -> Parts<T> {
        let (io, read_buf, _) = match self.inner.expect("already upgraded") {
            Either::Left(h1) => h1.into_inner(),
            Either::Right(_h2) => {
                panic!("http2 cannot into_inner");
            }
        };

        Parts {
            io: io,
            read_buf: read_buf,
            _inner: (),
        }
    }

    /// Poll the connection for completion, but without calling `shutdown`
    /// on the underlying IO.
    ///
    /// This is useful to allow running a connection while doing an HTTP
    /// upgrade. Once the upgrade is completed, the connection would be "done",
    /// but it is not desired to actally shutdown the IO object. Instead you
    /// would take it back using `into_parts`.
    ///
    /// Use [`poll_fn`](https://docs.rs/futures/0.1.25/futures/future/fn.poll_fn.html)
    /// and [`try_ready!`](https://docs.rs/futures/0.1.25/futures/macro.try_ready.html)
    /// to work with this function; or use the `without_shutdown` wrapper.
    pub fn poll_without_shutdown(&mut self, cx: &mut task::Context<'_>) -> Poll<crate::Result<()>>
    where
        B: Unpin,
    {
        match self.inner.as_mut().expect("already upgraded") {
            &mut Either::Left(ref mut h1) => {
                h1.poll_without_shutdown(cx)
            },
            &mut Either::Right(ref mut h2) => {
                unimplemented!("h2 poll_without_shutdown");
                /*
                h2.poll().map(|x| x.map(|_| ()))
                */
            }
        }
    }

    /// Prevent shutdown of the underlying IO object at the end of service the request,
    /// instead run `into_parts`. This is a convenience wrapper over `poll_without_shutdown`.
    pub fn without_shutdown(self) -> impl Future<Output=crate::Result<Parts<T>>>
    where
        B: Unpin,
    {
        let mut conn = Some(self);
        future::poll_fn(move |cx| -> Poll<crate::Result<Parts<T>>> {
            ready!(conn.as_mut().unwrap().poll_without_shutdown(cx))?;
            Poll::Ready(Ok(conn.take().unwrap().into_parts()))
        })
    }
}

impl<T, B> Future for Connection<T, B>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    B: Payload + Unpin + 'static,
{
    type Output = crate::Result<()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Self::Output> {
        match ready!(Pin::new(self.inner.as_mut().unwrap()).poll(cx))? {
            proto::Dispatched::Shutdown => {
                Poll::Ready(Ok(()))
            },
            proto::Dispatched::Upgrade(pending) => {
                let h1 = match mem::replace(&mut self.inner, None) {
                    Some(Either::Left(h1)) => h1,
                    _ => unreachable!("Upgrade expects h1"),
                };

                let (io, buf, _) = h1.into_inner();
                pending.fulfill(Upgraded::new(Box::new(io), buf));
                Poll::Ready(Ok(()))
            }
        }
    }
}

impl<T, B> fmt::Debug for Connection<T, B>
where
    T: AsyncRead + AsyncWrite + fmt::Debug + Send + 'static,
    B: Payload + 'static,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Connection")
            .finish()
    }
}

// ===== impl Builder

impl Builder {
    /// Creates a new connection builder.
    #[inline]
    pub fn new() -> Builder {
        let mut h2_builder = h2::client::Builder::default();
        h2_builder.enable_push(false);

        Builder {
            exec: Exec::Default,
            h1_writev: true,
            h1_read_buf_exact_size: None,
            h1_title_case_headers: false,
            h1_max_buf_size: None,
            http2: false,
            h2_builder,
        }
    }

    /// Provide an executor to execute background HTTP2 tasks.
    pub fn executor<E>(&mut self, exec: E) -> &mut Builder
    where
        for<'a> &'a E: tokio_executor::Executor,
        E: Send + Sync + 'static,
    {
        self.exec = Exec::Executor(Arc::new(exec));
        self
    }

    pub(super) fn h1_writev(&mut self, enabled: bool) -> &mut Builder {
        self.h1_writev = enabled;
        self
    }

    pub(super) fn h1_title_case_headers(&mut self, enabled: bool) -> &mut Builder {
        self.h1_title_case_headers = enabled;
        self
    }

    pub(super) fn h1_read_buf_exact_size(&mut self, sz: Option<usize>) -> &mut Builder {
        self.h1_read_buf_exact_size = sz;
        self.h1_max_buf_size = None;
        self
    }

    pub(super) fn h1_max_buf_size(&mut self, max: usize) -> &mut Self {
        assert!(
            max >= proto::h1::MINIMUM_MAX_BUFFER_SIZE,
            "the max_buf_size cannot be smaller than the minimum that h1 specifies."
        );

        self.h1_max_buf_size = Some(max);
        self.h1_read_buf_exact_size = None;
        self
    }

    /// Sets whether HTTP2 is required.
    ///
    /// Default is false.
    pub fn http2_only(&mut self, enabled: bool) -> &mut Builder {
        self.http2 = enabled;
        self
    }

    /// Sets the [`SETTINGS_INITIAL_WINDOW_SIZE`][spec] option for HTTP2
    /// stream-level flow control.
    ///
    /// Default is 65,535
    ///
    /// [spec]: https://http2.github.io/http2-spec/#SETTINGS_INITIAL_WINDOW_SIZE
    pub fn http2_initial_stream_window_size(&mut self, sz: impl Into<Option<u32>>) -> &mut Self {
        if let Some(sz) = sz.into() {
            self.h2_builder.initial_window_size(sz);
        }
        self
    }

    /// Sets the max connection-level flow control for HTTP2
    ///
    /// Default is 65,535
    pub fn http2_initial_connection_window_size(&mut self, sz: impl Into<Option<u32>>) -> &mut Self {
        if let Some(sz) = sz.into() {
            self.h2_builder.initial_connection_window_size(sz);
        }
        self
    }

    /// Constructs a connection with the configured options and IO.
    #[inline]
    pub fn handshake<T, B>(&self, io: T) -> Handshake<T, B>
    where
        T: AsyncRead + AsyncWrite + Unpin + Send + 'static,
        B: Payload + 'static,
    {
        trace!("client handshake HTTP/{}", if self.http2 { 2 } else { 1 });
        Handshake {
            builder: self.clone(),
            io: Some(io),
            _marker: PhantomData,
        }
    }
}

// ===== impl Handshake

impl<T, B> Future for Handshake<T, B>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    B: Payload + 'static,
{
    type Output = crate::Result<(SendRequest<B>, Connection<T, B>)>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Self::Output> {
        let io = self.io.take().expect("polled more than once");
        let (tx, rx) = dispatch::channel();
        let either = if !self.builder.http2 {
            let mut conn = proto::Conn::new(io);
            if !self.builder.h1_writev {
                conn.set_write_strategy_flatten();
            }
            if self.builder.h1_title_case_headers {
                conn.set_title_case_headers();
            }
            if let Some(sz) = self.builder.h1_read_buf_exact_size {
                conn.set_read_buf_exact_size(sz);
            }
            if let Some(max) = self.builder.h1_max_buf_size {
                conn.set_max_buf_size(max);
            }
            let cd = proto::h1::dispatch::Client::new(rx);
            let dispatch = proto::h1::Dispatcher::new(cd, conn);
            Either::Left(dispatch)
        } else {
            let h2 = proto::h2::Client::new(io, rx, &self.builder.h2_builder, self.builder.exec.clone());
            Either::Right(h2)
        };

        Poll::Ready(Ok((
            SendRequest {
                dispatch: tx,
            },
            Connection {
                inner: Some(either),
            },
        )))
    }
}

impl<T, B> fmt::Debug for Handshake<T, B> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Handshake")
            .finish()
    }
}

// ===== impl ResponseFuture

impl Future for ResponseFuture {
    type Output = crate::Result<Response<Body>>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Self::Output> {
        match self.inner {
            ResponseFutureState::Waiting(ref mut rx) => {
                Pin::new(rx).poll(cx).map(|res| match res {
                    Ok(Ok(resp)) => Ok(resp),
                    Ok(Err(err)) => Err(err),
                    // this is definite bug if it happens, but it shouldn't happen!
                    Err(_canceled) => panic!("dispatch dropped without returning error"),
                })
            },
            ResponseFutureState::Error(ref mut err) => {
                Poll::Ready(Err(err.take().expect("polled after ready")))
            }
        }
    }
}

impl fmt::Debug for ResponseFuture {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("ResponseFuture")
            .finish()
    }
}

// assert trait markers

trait AssertSend: Send {}
trait AssertSendSync: Send + Sync {}


#[doc(hidden)]
impl<B: Send> AssertSendSync for SendRequest<B> {}

#[doc(hidden)]
impl<T: Send, B: Send> AssertSend for Connection<T, B>
where
    T: AsyncRead + AsyncWrite + Send + 'static,
    B: Payload + 'static,
{}

#[doc(hidden)]
impl<T: Send + Sync, B: Send + Sync> AssertSendSync for Connection<T, B>
where
    T: AsyncRead + AsyncWrite + Send + 'static,
    B: Payload + 'static,
    B::Data: Send + Sync + 'static,
{}

#[doc(hidden)]
impl AssertSendSync for Builder {}

#[doc(hidden)]
impl AssertSend for ResponseFuture {}

