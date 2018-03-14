//! Lower-level client connection API.
//!
//! The types in thie module are to provide a lower-level API based around a
//! single connection. Connecting to a host, pooling connections, and the like
//! are not handled at this level. This module provides the building blocks to
//! customize those things externally.
//!
//! If don't have need to manage connections yourself, consider using the
//! higher-level [Client](super) API.
use std::fmt;
use std::marker::PhantomData;

use bytes::Bytes;
use futures::{Async, Future, Poll};
use futures::future::{self, Either};
use tokio_io::{AsyncRead, AsyncWrite};

use proto;
use proto::body::Entity;
use super::dispatch;
use {Body, Request, Response, StatusCode};

/// Returns a `Handshake` future over some IO.
///
/// This is a shortcut for `Builder::new().handshake(io)`.
pub fn handshake<T>(io: T) -> Handshake<T, ::Body>
where
    T: AsyncRead + AsyncWrite,
{
    Builder::new()
        .handshake(io)
}

/// The sender side of an established connection.
pub struct SendRequest<B> {
    dispatch: dispatch::Sender<proto::dispatch::ClientMsg<B>, Response<Body>>,

}

/// A future that processes all HTTP state for the IO object.
///
/// In most cases, this should just be spawned into an executor, so that it
/// can process incoming and outgoing messages, notice hangups, and the like.
#[must_use = "futures do nothing unless polled"]
pub struct Connection<T, B>
where
    T: AsyncRead + AsyncWrite,
    B: Entity<Error=::Error> + 'static,
{
    inner: proto::dispatch::Dispatcher<
        proto::dispatch::Client<B>,
        B,
        T,
        B::Data,
        proto::ClientUpgradeTransaction,
    >,
}


/// A builder to configure an HTTP connection.
///
/// After setting options, the builder is used to create a `Handshake` future.
#[derive(Clone, Debug)]
pub struct Builder {
    h1_writev: bool,
}

/// A future setting up HTTP over an IO object.
///
/// If successful, yields a `(SendRequest, Connection)` pair.
#[must_use = "futures do nothing unless polled"]
pub struct Handshake<T, B> {
    inner: HandshakeInner<T, B, proto::ClientUpgradeTransaction>,
}

/// A future returned by `SendRequest::send_request`.
///
/// Yields a `Response` if successful.
#[must_use = "futures do nothing unless polled"]
pub struct ResponseFuture {
    // for now, a Box is used to hide away the internal `B`
    // that can be returned if canceled
    inner: Box<Future<Item=Response<Body>, Error=::Error> + Send>,
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

// internal client api

#[must_use = "futures do nothing unless polled"]
pub(super) struct HandshakeNoUpgrades<T, B> {
    inner: HandshakeInner<T, B, proto::ClientTransaction>,
}

struct HandshakeInner<T, B, R> {
    builder: Builder,
    io: Option<T>,
    _marker: PhantomData<(B, R)>,
}

// ===== impl SendRequest

impl<B> SendRequest<B>
{
    /// Polls to determine whether this sender can be used yet for a request.
    ///
    /// If the associated connection is closed, this returns an Error.
    pub fn poll_ready(&mut self) -> Poll<(), ::Error> {
        self.dispatch.poll_ready()
    }

    pub(super) fn is_closed(&self) -> bool {
        self.dispatch.is_closed()
    }
}

impl<B> SendRequest<B>
where
    B: Entity<Error=::Error> + 'static,
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
    ///   authority are need to connect. They aren't required here.
    /// - Since the `Client` requires absolute-form `Uri`s, it can add
    ///   the `Host` header based on it. You must add a `Host` header yourself
    ///   before calling this method.
    /// - Since absolute-form `Uri`s are not required, if received, they will
    ///   be serialized as-is.
    ///
    /// # Example
    ///
    /// ```
    /// # extern crate futures;
    /// # extern crate hyper;
    /// # extern crate http;
    /// # use http::header::HOST;
    /// # use hyper::client::conn::SendRequest;
    /// # use hyper::Body;
    /// use futures::Future;
    /// use hyper::Request;
    ///
    /// # fn doc(mut tx: SendRequest<Body>) {
    /// // build a Request
    /// let req = Request::builder()
    ///     .uri("/foo/bar")
    ///     .header(HOST, "hyper.rs")
    ///     .body(Body::empty())
    ///     .unwrap();
    ///
    /// // send it and get a future back
    /// let fut = tx.send_request(req)
    ///     .map(|res| {
    ///         // got the Response
    ///         assert!(res.status().is_success());
    ///     });
    /// # drop(fut);
    /// # }
    /// # fn main() {}
    /// ```
    pub fn send_request(&mut self, req: Request<B>) -> ResponseFuture {
        let inner = match self.dispatch.send(req) {
            Ok(rx) => {
                Either::A(rx.then(move |res| {
                    match res {
                        Ok(Ok(res)) => Ok(res),
                        Ok(Err(err)) => Err(err),
                        // this is definite bug if it happens, but it shouldn't happen!
                        Err(_) => panic!("dispatch dropped without returning error"),
                    }
                }))
            },
            Err(_req) => {
                debug!("connection was not ready");
                let err = ::Error::new_canceled(Some("connection was not ready"));
                Either::B(future::err(err))
            }
        };

        ResponseFuture {
            inner: Box::new(inner),
        }
    }

    //TODO: replace with `impl Future` when stable
    pub(crate) fn send_request_retryable(&mut self, req: Request<B>) -> Box<Future<Item=Response<Body>, Error=(::Error, Option<Request<B>>)> + Send>
    where
        B: Send,
    {
        let inner = match self.dispatch.try_send(req) {
            Ok(rx) => {
                Either::A(rx.then(move |res| {
                    match res {
                        Ok(Ok(res)) => Ok(res),
                        Ok(Err(err)) => Err(err),
                        // this is definite bug if it happens, but it shouldn't happen!
                        Err(_) => panic!("dispatch dropped without returning error"),
                    }
                }))
            },
            Err(req) => {
                debug!("connection was not ready");
                let err = ::Error::new_canceled(Some("connection was not ready"));
                Either::B(future::err((err, Some(req))))
            }
        };
        Box::new(inner)
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

// ===== impl Connection

impl<T, B> Connection<T, B>
where
    T: AsyncRead + AsyncWrite,
    B: Entity<Error=::Error> + 'static,
{
    /// Return the inner IO object, and additional information.
    pub fn into_parts(self) -> Parts<T> {
        let (io, read_buf) = self.inner.into_inner();
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
    pub fn poll_without_shutdown(&mut self) -> Poll<(), ::Error> {
        self.inner.poll_without_shutdown()
    }
}

impl<T, B> Future for Connection<T, B>
where
    T: AsyncRead + AsyncWrite,
    B: Entity<Error=::Error> + 'static,
{
    type Item = ();
    type Error = ::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        self.inner.poll()
    }
}

impl<T, B> fmt::Debug for Connection<T, B>
where
    T: AsyncRead + AsyncWrite + fmt::Debug,
    B: Entity<Error=::Error> + 'static,
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
        Builder {
            h1_writev: true,
        }
    }

    pub(super) fn h1_writev(&mut self, enabled: bool) -> &mut Builder {
        self.h1_writev = enabled;
        self
    }

    /// Constructs a connection with the configured options and IO.
    #[inline]
    pub fn handshake<T, B>(&self, io: T) -> Handshake<T, B>
    where
        T: AsyncRead + AsyncWrite,
        B: Entity<Error=::Error> + 'static,
    {
        Handshake {
            inner: HandshakeInner {
                builder: self.clone(),
                io: Some(io),
                _marker: PhantomData,
            }
        }
    }

    pub(super) fn handshake_no_upgrades<T, B>(&self, io: T) -> HandshakeNoUpgrades<T, B>
    where
        T: AsyncRead + AsyncWrite,
        B: Entity<Error=::Error> + 'static,
    {
        HandshakeNoUpgrades {
            inner: HandshakeInner {
                builder: self.clone(),
                io: Some(io),
                _marker: PhantomData,
            }
        }
    }
}

// ===== impl Handshake

impl<T, B> Future for Handshake<T, B>
where
    T: AsyncRead + AsyncWrite,
    B: Entity<Error=::Error> + 'static,
{
    type Item = (SendRequest<B>, Connection<T, B>);
    type Error = ::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        self.inner.poll()
            .map(|async| {
                async.map(|(tx, dispatch)| {
                    (tx, Connection { inner: dispatch })
                })
            })
    }
}

impl<T, B> fmt::Debug for Handshake<T, B> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Handshake")
            .finish()
    }
}

impl<T, B> Future for HandshakeNoUpgrades<T, B>
where
    T: AsyncRead + AsyncWrite,
    B: Entity<Error=::Error> + 'static,
{
    type Item = (SendRequest<B>, proto::dispatch::Dispatcher<
        proto::dispatch::Client<B>,
        B,
        T,
        B::Data,
        proto::ClientTransaction,
    >);
    type Error = ::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        self.inner.poll()
    }
}

impl<T, B, R> Future for HandshakeInner<T, B, R>
where
    T: AsyncRead + AsyncWrite,
    B: Entity<Error=::Error> + 'static,
    R: proto::Http1Transaction<
        Incoming=StatusCode,
        Outgoing=proto::RequestLine,
    >,
{
    type Item = (SendRequest<B>, proto::dispatch::Dispatcher<
        proto::dispatch::Client<B>,
        B,
        T,
        B::Data,
        R,
    >);
    type Error = ::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let io = self.io.take().expect("polled more than once");
        let (tx, rx) = dispatch::channel();
        let mut conn = proto::Conn::new(io);
        if !self.builder.h1_writev {
            conn.set_write_strategy_flatten();
        }
        let dispatch = proto::dispatch::Dispatcher::new(proto::dispatch::Client::new(rx), conn);
        Ok(Async::Ready((
            SendRequest {
                dispatch: tx,
            },
            dispatch,
        )))
    }
}

// ===== impl ResponseFuture

impl Future for ResponseFuture {
    type Item = Response<Body>;
    type Error = ::Error;

    #[inline]
    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        self.inner.poll()
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
    T: AsyncRead + AsyncWrite,
    B: Entity<Error=::Error> + 'static,
    B::Data: Send + 'static,
{}

#[doc(hidden)]
impl<T: Send + Sync, B: Send + Sync> AssertSendSync for Connection<T, B>
where
    T: AsyncRead + AsyncWrite,
    B: Entity<Error=::Error> + 'static,
    B::Data: Send + Sync + 'static,
{}

#[doc(hidden)]
impl AssertSendSync for Builder {}

#[doc(hidden)]
impl AssertSend for ResponseFuture {}

