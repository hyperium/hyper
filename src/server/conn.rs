//! Lower-level Server connection API.
//!
//! The types in this module are to provide a lower-level API based around a
//! single connection. Accepting a connection and binding it with a service
//! are not handled at this level. This module provides the building blocks to
//! customize those things externally.
//!
//! If you don't have need to manage connections yourself, consider using the
//! higher-level [Server](super) API.
//!
//! ## Example
//! A simple example that uses the `Http` struct to talk HTTP over a Tokio TCP stream
//! ```no_run
//! # #[cfg(feature = "runtime")]
//! # mod rt {
//! use http::{Request, Response, StatusCode};
//! use hyper::{server::conn::Http, service::service_fn, Body};
//! use std::{net::SocketAddr, convert::Infallible};
//! use tokio::net::TcpListener;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
//!     let addr: SocketAddr = ([127, 0, 0, 1], 8080).into();
//!
//!     let mut tcp_listener = TcpListener::bind(addr).await?;
//!     loop {
//!         let (tcp_stream, _) = tcp_listener.accept().await?;
//!         tokio::task::spawn(async move {
//!             if let Err(http_err) = Server::new()
//!                     .http1_only(true)
//!                     .keep_alive(true)
//!                     .serve_connection(tcp_stream, service_fn(hello))
//!                     .await {
//!                 eprintln!("Error while serving HTTP connection: {}", http_err);
//!             }
//!         });
//!     }
//! }
//!
//! async fn hello(_req: Request<Body>) -> Result<Response<Body>, Infallible> {
//!    Ok(Response::new(Body::from("Hello World!")))
//! }
//! # }
//! ```

use std::error::Error as StdError;
use std::fmt;
use std::mem;

use bytes::Bytes;
use pin_project::pin_project;
use tokio::io::{AsyncRead, AsyncWrite};

use crate::body::{Body, HttpBody};
use crate::common::exec::{Exec, H2Exec};
use crate::common::io::Rewind;
use crate::common::{task, Future, Pin, Poll, Unpin};
use crate::error::{Kind, Parse};
use crate::proto;
use crate::service::HttpService;
use crate::upgrade::Upgraded;

pub(super) use self::upgrades::UpgradeableConnection;

// #[cfg(feature = "tcp")]
// pub use super::tcp::{AddrIncoming, AddrStream};

/// A future binding a connection with a Service.
///
/// Polling this future will drive HTTP forward.
#[must_use = "futures do nothing unless polled"]
#[pin_project]
pub struct Connection<T, S, E = Exec>
where
    S: HttpService<Body>,
{
    pub(super) conn: Option<ProtoServer<T, S::ResBody, S, E>>,
    pub(super) fallback: Fallback<E>,
}

#[pin_project(project = ProtoServerProj)]
pub(super) enum ProtoServer<T, B, S, E = Exec>
where
    S: HttpService<Body>,
    B: HttpBody,
{
    H1(
        #[pin]
        proto::h1::Dispatcher<
            proto::h1::dispatch::Server<S, Body>,
            B,
            T,
            proto::ServerTransaction,
        >,
    ),
    H2(#[pin] proto::h2::Server<Rewind<T>, S, B, E>),
}

#[derive(Clone, Debug)]
pub(super) enum Fallback<E> {
    ToHttp2(proto::h2::server::Config, E),
    Http1Only,
}

impl<E> Fallback<E> {
    fn to_h2(&self) -> bool {
        match *self {
            Fallback::ToHttp2(..) => true,
            Fallback::Http1Only => false,
        }
    }
}

impl<E> Unpin for Fallback<E> {}

/// Deconstructed parts of a `Connection`.
///
/// This allows taking apart a `Connection` at a later time, in order to
/// reclaim the IO object, and additional related pieces.
#[derive(Debug)]
pub struct Parts<T, S> {
    /// The original IO object used in the handshake.
    pub io: T,
    /// A buffer of bytes that have been read but not processed as HTTP.
    ///
    /// If the client sent additional bytes after its last request, and
    /// this connection "ended" with an upgrade, the read buffer will contain
    /// those bytes.
    ///
    /// You will want to check for any existing bytes if you plan to continue
    /// communicating on the IO object.
    pub read_buf: Bytes,
    /// The `Service` used to serve this connection.
    pub service: S,
    _inner: (),
}

// ===== impl Connection =====

impl<I, B, S, E> Connection<I, S, E>
where
    S: HttpService<Body, ResBody = B>,
    S::Error: Into<Box<dyn StdError + Send + Sync>>,
    I: AsyncRead + AsyncWrite + Unpin,
    B: HttpBody + 'static,
    B::Error: Into<Box<dyn StdError + Send + Sync>>,
    E: H2Exec<S::Future, B>,
{
    /// Start a graceful shutdown process for this connection.
    ///
    /// This `Connection` should continue to be polled until shutdown
    /// can finish.
    ///
    /// # Note
    ///
    /// This should only be called while the `Connection` future is still
    /// pending. If called after `Connection::poll` has resolved, this does
    /// nothing.
    pub fn graceful_shutdown(self: Pin<&mut Self>) {
        match self.project().conn {
            Some(ProtoServer::H1(ref mut h1)) => {
                h1.disable_keep_alive();
            }
            Some(ProtoServer::H2(ref mut h2)) => {
                h2.graceful_shutdown();
            }
            None => (),
        }
    }

    /// Return the inner IO object, and additional information.
    ///
    /// If the IO object has been "rewound" the io will not contain those bytes rewound.
    /// This should only be called after `poll_without_shutdown` signals
    /// that the connection is "done". Otherwise, it may not have finished
    /// flushing all necessary HTTP bytes.
    ///
    /// # Panics
    /// This method will panic if this connection is using an h2 protocol.
    pub fn into_parts(self) -> Parts<I, S> {
        self.try_into_parts()
            .unwrap_or_else(|| panic!("h2 cannot into_inner"))
    }

    /// Return the inner IO object, and additional information, if available.
    ///
    /// This method will return a `None` if this connection is using an h2 protocol.
    pub fn try_into_parts(self) -> Option<Parts<I, S>> {
        match self.conn.unwrap() {
            ProtoServer::H1(h1) => {
                let (io, read_buf, dispatch) = h1.into_inner();
                Some(Parts {
                    io,
                    read_buf,
                    service: dispatch.into_service(),
                    _inner: (),
                })
            }
            ProtoServer::H2(_h2) => None,
        }
    }

    /// Poll the connection for completion, but without calling `shutdown`
    /// on the underlying IO.
    ///
    /// This is useful to allow running a connection while doing an HTTP
    /// upgrade. Once the upgrade is completed, the connection would be "done",
    /// but it is not desired to actually shutdown the IO object. Instead you
    /// would take it back using `into_parts`.
    ///
    /// Use [`poll_fn`](https://docs.rs/futures/0.1.25/futures/future/fn.poll_fn.html)
    /// and [`try_ready!`](https://docs.rs/futures/0.1.25/futures/macro.try_ready.html)
    /// to work with this function; or use the `without_shutdown` wrapper.
    pub fn poll_without_shutdown(&mut self, cx: &mut task::Context<'_>) -> Poll<crate::Result<()>>
    where
        S: Unpin,
        S::Future: Unpin,
        B: Unpin,
    {
        loop {
            let polled = match *self.conn.as_mut().unwrap() {
                ProtoServer::H1(ref mut h1) => h1.poll_without_shutdown(cx),
                ProtoServer::H2(ref mut h2) => return Pin::new(h2).poll(cx).map_ok(|_| ()),
            };
            match ready!(polled) {
                Ok(()) => return Poll::Ready(Ok(())),
                Err(e) => match *e.kind() {
                    Kind::Parse(Parse::VersionH2) if self.fallback.to_h2() => {
                        self.upgrade_h2();
                        continue;
                    }
                    _ => return Poll::Ready(Err(e)),
                },
            }
        }
    }

    /// Prevent shutdown of the underlying IO object at the end of service the request,
    /// instead run `into_parts`. This is a convenience wrapper over `poll_without_shutdown`.
    pub fn without_shutdown(self) -> impl Future<Output = crate::Result<Parts<I, S>>>
    where
        S: Unpin,
        S::Future: Unpin,
        B: Unpin,
    {
        let mut conn = Some(self);
        futures_util::future::poll_fn(move |cx| {
            ready!(conn.as_mut().unwrap().poll_without_shutdown(cx))?;
            Poll::Ready(Ok(conn.take().unwrap().into_parts()))
        })
    }

    fn upgrade_h2(&mut self) {
        trace!("Trying to upgrade connection to h2");
        let conn = self.conn.take();

        let (io, read_buf, dispatch) = match conn.unwrap() {
            ProtoServer::H1(h1) => h1.into_inner(),
            ProtoServer::H2(_h2) => {
                panic!("h2 cannot into_inner");
            }
        };
        let mut rewind_io = Rewind::new(io);
        rewind_io.rewind(read_buf);
        let (builder, exec) = match self.fallback {
            Fallback::ToHttp2(ref builder, ref exec) => (builder, exec),
            Fallback::Http1Only => unreachable!("upgrade_h2 with Fallback::Http1Only"),
        };
        let h2 = proto::h2::Server::new(rewind_io, dispatch.into_service(), builder, exec.clone());

        debug_assert!(self.conn.is_none());
        self.conn = Some(ProtoServer::H2(h2));
    }

    /// Enable this connection to support higher-level HTTP upgrades.
    ///
    /// See [the `upgrade` module](crate::upgrade) for more.
    pub fn with_upgrades(self) -> UpgradeableConnection<I, S, E>
    where
        I: Send,
    {
        UpgradeableConnection { inner: self }
    }
}

impl<I, B, S, E> Future for Connection<I, S, E>
where
    S: HttpService<Body, ResBody = B>,
    S::Error: Into<Box<dyn StdError + Send + Sync>>,
    I: AsyncRead + AsyncWrite + Unpin + 'static,
    B: HttpBody + 'static,
    B::Error: Into<Box<dyn StdError + Send + Sync>>,
    E: H2Exec<S::Future, B>,
{
    type Output = crate::Result<()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Self::Output> {
        loop {
            match ready!(Pin::new(self.conn.as_mut().unwrap()).poll(cx)) {
                Ok(done) => {
                    if let proto::Dispatched::Upgrade(pending) = done {
                        // With no `Send` bound on `I`, we can't try to do
                        // upgrades here. In case a user was trying to use
                        // `Body::on_upgrade` with this API, send a special
                        // error letting them know about that.
                        pending.manual();
                    }
                    return Poll::Ready(Ok(()));
                }
                Err(e) => match *e.kind() {
                    Kind::Parse(Parse::VersionH2) if self.fallback.to_h2() => {
                        self.upgrade_h2();
                        continue;
                    }
                    _ => return Poll::Ready(Err(e)),
                },
            }
        }
    }
}

impl<I, S> fmt::Debug for Connection<I, S>
where
    S: HttpService<Body>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Connection").finish()
    }
}

// ===== impl ProtoServer =====

impl<T, B, S, E> Future for ProtoServer<T, B, S, E>
where
    T: AsyncRead + AsyncWrite + Unpin,
    S: HttpService<Body, ResBody = B>,
    S::Error: Into<Box<dyn StdError + Send + Sync>>,
    B: HttpBody + 'static,
    B::Error: Into<Box<dyn StdError + Send + Sync>>,
    E: H2Exec<S::Future, B>,
{
    type Output = crate::Result<proto::Dispatched>;

    fn poll(self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Self::Output> {
        match self.project() {
            ProtoServerProj::H1(s) => s.poll(cx),
            ProtoServerProj::H2(s) => s.poll(cx),
        }
    }
}

pub(crate) mod spawn_all {
    use std::error::Error as StdError;
    use tokio::io::{AsyncRead, AsyncWrite};

    // use super::UpgradeableConnection;
    use super::Connection;
    use crate::body::{Body, HttpBody};
    use crate::common::exec::H2Exec;
    use crate::common::{task, Future, Pin, Poll, Unpin};
    use crate::service::HttpService;
    use pin_project::pin_project;

    // This is a `Future<Item=(), Error=()>` spawned to an `Executor` inside
    // the `SpawnAll`. By being a nameable type, we can be generic over the
    // user's `Service::Future`, and thus an `Executor` can execute it.
    //
    // Doing this allows for the server to conditionally require `Send` futures,
    // depending on the `Executor` configured.
    //
    // Users cannot import this type, nor the associated `NewSvcExec`. Instead,
    // a blanket implementation for `Executor<impl Future>` is sufficient.

    #[pin_project]
    #[allow(missing_debug_implementations)]
    pub struct SvcTask<I, S: HttpService<Body>, E> {
        #[pin]
        conn: Connection<I, S, E>,
    }

    impl<I, S: HttpService<Body>, E> SvcTask<I, S, E> {
        pub(crate) fn new(conn: Connection<I, S, E>) -> Self {
            SvcTask { conn }
        }
    }

    impl<I, S, B, E> Future for SvcTask<I, S, E>
    where
        I: AsyncRead + AsyncWrite + Unpin + Send + 'static,
        S: HttpService<Body, ResBody = B>,
        B: HttpBody + 'static,
        B::Error: Into<Box<dyn StdError + Send + Sync>>,
        E: H2Exec<S::Future, B>,
    {
        type Output = ();

        fn poll(self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Self::Output> {
            // If it weren't for needing to name this type so the `Send` bounds
            // could be projected to the `Serve` executor, this could just be
            // an `async fn`, and much safer. Woe is me.

            return self.project().conn.poll(cx).map(|res| {
                if let Err(err) = res {
                    debug!("connection error: {}", err);
                }
            });
        }
    }
}

mod upgrades {
    use super::*;

    // A future binding a connection with a Service with Upgrade support.
    //
    // This type is unnameable outside the crate, and so basically just an
    // `impl Future`, without requiring Rust 1.26.
    #[must_use = "futures do nothing unless polled"]
    #[allow(missing_debug_implementations)]
    pub struct UpgradeableConnection<T, S, E>
    where
        S: HttpService<Body>,
    {
        pub(super) inner: Connection<T, S, E>,
    }

    impl<I, B, S, E> UpgradeableConnection<I, S, E>
    where
        S: HttpService<Body, ResBody = B>,
        S::Error: Into<Box<dyn StdError + Send + Sync>>,
        I: AsyncRead + AsyncWrite + Unpin,
        B: HttpBody + 'static,
        B::Error: Into<Box<dyn StdError + Send + Sync>>,
        E: H2Exec<S::Future, B>,
    {
        /// Start a graceful shutdown process for this connection.
        ///
        /// This `Connection` should continue to be polled until shutdown
        /// can finish.
        pub fn graceful_shutdown(mut self: Pin<&mut Self>) {
            Pin::new(&mut self.inner).graceful_shutdown()
        }
    }

    impl<I, B, S, E> Future for UpgradeableConnection<I, S, E>
    where
        S: HttpService<Body, ResBody = B>,
        S::Error: Into<Box<dyn StdError + Send + Sync>>,
        I: AsyncRead + AsyncWrite + Unpin + Send + 'static,
        B: HttpBody + 'static,
        B::Error: Into<Box<dyn StdError + Send + Sync>>,
        E: super::H2Exec<S::Future, B>,
    {
        type Output = crate::Result<()>;

        fn poll(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Self::Output> {
            loop {
                match ready!(Pin::new(self.inner.conn.as_mut().unwrap()).poll(cx)) {
                    Ok(proto::Dispatched::Shutdown) => return Poll::Ready(Ok(())),
                    Ok(proto::Dispatched::Upgrade(pending)) => {
                        let h1 = match mem::replace(&mut self.inner.conn, None) {
                            Some(ProtoServer::H1(h1)) => h1,
                            _ => unreachable!("Upgrade expects h1"),
                        };

                        let (io, buf, _) = h1.into_inner();
                        pending.fulfill(Upgraded::new(io, buf));
                        return Poll::Ready(Ok(()));
                    }
                    Err(e) => match *e.kind() {
                        Kind::Parse(Parse::VersionH2) if self.inner.fallback.to_h2() => {
                            self.inner.upgrade_h2();
                            continue;
                        }
                        _ => return Poll::Ready(Err(e)),
                    },
                }
            }
        }
    }
}
