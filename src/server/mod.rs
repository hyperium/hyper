//! HTTP Server
//!
//! A `Server` is created to listen on a port, parse HTTP requests, and hand
//! them off to a `Service`.
//!
//! There are two levels of APIs provide for constructing HTTP servers:
//!
//! - The higher-level [`Server`](Server) type.
//! - The lower-level [conn](conn) module.
//!
//! # Server
//!
//! The [`Server`](Server) is main way to start listening for HTTP requests.
//! It wraps a listener with a [`NewService`](::service), and then should
//! be executed to start serving requests.
//!
//! [`Server`](Server) accepts connections in both HTTP1 and HTTP2 by default.
//!
//! ## Example
//!
//! ```no_run
//! extern crate hyper;
//!
//! use hyper::{Body, Response, Server};
//! use hyper::service::service_fn_ok;
//!
//! # #[cfg(feature = "runtime")]
//! fn main() {
//! # use hyper::rt::Future;
//!     // Construct our SocketAddr to listen on...
//!     let addr = ([127, 0, 0, 1], 3000).into();
//!
//!     // And a NewService to handle each connection...
//!     let new_service = || {
//!         service_fn_ok(|_req| {
//!             Response::new(Body::from("Hello World"))
//!         })
//!     };
//!
//!     // Then bind and serve...
//!     let server = Server::bind(&addr)
//!         .serve(new_service);
//!
//!     // Finally, spawn `server` onto an Executor...
//!     hyper::rt::run(server.map_err(|e| {
//!         eprintln!("server error: {}", e);
//!     }));
//! }
//! # #[cfg(not(feature = "runtime"))]
//! # fn main() {}
//! ```

pub mod conn;
mod shutdown;
#[cfg(feature = "runtime")] mod tcp;
#[cfg(all(unix,feature = "runtime"))] mod uds;

use std::fmt;
#[cfg(feature = "runtime")] use std::net::{SocketAddr, TcpListener as StdTcpListener};

#[cfg(all(unix,feature = "runtime"))] use std::os::unix::net::{SocketAddr as UnixSocketAddr,
    UnixListener as StdUnixListener};

#[cfg(feature = "runtime")] use std::time::Duration;

use futures::{Future, Stream, Poll};
use tokio_io::{AsyncRead, AsyncWrite};
#[cfg(feature = "runtime")] use tokio_reactor;

use body::{Body, Payload};
use common::exec::{Exec, H2Exec, NewSvcExec};
use service::{NewService, Service};
// Renamed `Http` as `Http_` for now so that people upgrading don't see an
// error that `hyper::server::Http` is private...
use self::conn::{Http as Http_, NoopWatcher, SpawnAll};
use self::shutdown::{Graceful, GracefulWatcher};
#[cfg(feature = "runtime")] use self::tcp::AddrIncoming;
#[cfg(all(unix,feature = "runtime"))] use self::uds::UnixIncoming;
#[cfg(all(unix,feature = "runtime"))] use std::path::Path;

/// A listening HTTP server that accepts connections in both HTTP1 and HTTP2 by default.
///
/// `Server` is a `Future` mapping a bound listener with a set of service
/// handlers. It is built using the [`Builder`](Builder), and the future
/// completes when the server has been shutdown. It should be run by an
/// `Executor`.
pub struct Server<I, S, E = Exec> {
    spawn_all: SpawnAll<I, S, E>,
}

/// A builder for a [`Server`](Server).
#[derive(Debug)]
pub struct Builder<I, E = Exec> {
    incoming: I,
    protocol: Http_<E>,
}

// ===== impl Server =====

impl<I> Server<I, ()> {
    /// Starts a [`Builder`](Builder) with the provided incoming stream.
    pub fn builder(incoming: I) -> Builder<I> {
        Builder {
            incoming,
            protocol: Http_::new(),
        }
    }
}

#[cfg(feature = "runtime")]
impl Server<AddrIncoming, ()> {
    /// Binds to the provided address, and returns a [`Builder`](Builder).
    ///
    /// # Panics
    ///
    /// This method will panic if binding to the address fails. For a method
    /// to bind to an address and return a `Result`, see `Server::try_bind`.
    pub fn bind(addr: &SocketAddr) -> Builder<AddrIncoming> {
        let incoming = AddrIncoming::new(addr, None)
            .unwrap_or_else(|e| {
                panic!("error binding to {}: {}", addr, e);
            });
        Server::builder(incoming)
    }

    /// Tries to bind to the provided address, and returns a [`Builder`](Builder).
    pub fn try_bind(addr: &SocketAddr) -> ::Result<Builder<AddrIncoming>> {
        AddrIncoming::new(addr, None)
            .map(Server::builder)
    }

    /// Create a new instance from a `std::net::TcpListener` instance.
    pub fn from_tcp(listener: StdTcpListener) -> Result<Builder<AddrIncoming>, ::Error> {
        let handle = tokio_reactor::Handle::current();
        AddrIncoming::from_std(listener, &handle)
            .map(Server::builder)
    }
}

#[cfg(feature = "runtime")]
impl<S> Server<AddrIncoming, S> {
    /// Returns the local address that this server is bound to.
    pub fn local_addr(&self) -> SocketAddr {
        self.spawn_all.local_addr()
    }
}

#[cfg(all(unix,feature = "runtime"))]
impl Server<UnixIncoming, ()> {
    /// Binds to the provided address, and returns a [`Builder`](Builder).
    ///
    /// # Panics
    ///
    /// This method will panic if binding to the address fails. For a method
    /// to bind to an address and return a `Result`, see `Server::try_bind`.
    pub fn bind_unix<P: AsRef<Path>>(path: P) -> Builder<UnixIncoming> {
        let incoming = UnixIncoming::new(path.as_ref(), None)
            .unwrap_or_else(|e| {
                panic!("error binding to {}: {}", path.as_ref().display(), e);
            });
        Server::builder(incoming)
    }

    /// Tries to bind to the provided address, and returns a [`Builder`](Builder).
    pub fn try_bind_unix<P: AsRef<Path>>(path: P) -> ::Result<Builder<UnixIncoming>> {
        UnixIncoming::new(path, None)
            .map(Server::builder)
    }

    /// Create a new instance from a `std::os::unix::net::UnixListener` instance.
    pub fn from_socket(listener: StdUnixListener) -> Result<Builder<UnixIncoming>, ::Error> {
        let handle = tokio_reactor::Handle::current();
        UnixIncoming::from_std(listener, &handle)
            .map(Server::builder)
    }
}

#[cfg(all(unix,feature = "runtime"))]
impl<S> Server<UnixIncoming, S> {
    /// Returns the local address that this server is bound to.
    pub fn local_addr(&self) -> &UnixSocketAddr {
        self.spawn_all.local_addr()
    }
}

impl<I, S, E, B> Server<I, S, E>
where
    I: Stream,
    I::Error: Into<Box<::std::error::Error + Send + Sync>>,
    I::Item: AsyncRead + AsyncWrite + Send + 'static,
    S: NewService<ReqBody=Body, ResBody=B>,
    S::Error: Into<Box<::std::error::Error + Send + Sync>>,
    S::Service: 'static,
    B: Payload,
    E: H2Exec<<S::Service as Service>::Future, B>,
    E: NewSvcExec<I::Item, S::Future, S::Service, E, GracefulWatcher>,
{
    /// Prepares a server to handle graceful shutdown when the provided future
    /// completes.
    ///
    /// # Example
    ///
    /// ```
    /// # extern crate hyper;
    /// # extern crate futures;
    /// # use futures::Future;
    /// # fn main() {}
    /// # #[cfg(feature = "runtime")]
    /// # fn run() {
    /// # use hyper::{Body, Response, Server};
    /// # use hyper::service::service_fn_ok;
    /// # let new_service = || {
    /// #     service_fn_ok(|_req| {
    /// #         Response::new(Body::from("Hello World"))
    /// #     })
    /// # };
    ///
    /// // Make a server from the previous examples...
    /// let server = Server::bind(&([127, 0, 0, 1], 3000).into())
    ///     .serve(new_service);
    ///
    /// // Prepare some signal for when the server should start
    /// // shutting down...
    /// let (tx, rx) = futures::sync::oneshot::channel::<()>();
    ///
    /// let graceful = server
    ///     .with_graceful_shutdown(rx)
    ///     .map_err(|err| eprintln!("server error: {}", err));
    ///
    /// // Spawn `server` onto an Executor...
    /// hyper::rt::spawn(graceful);
    ///
    /// // And later, trigger the signal by calling `tx.send(())`.
    /// let _ = tx.send(());
    /// # }
    /// ```
    pub fn with_graceful_shutdown<F>(self, signal: F) -> Graceful<I, S, F, E>
    where
        F: Future<Item=()>
    {
        Graceful::new(self.spawn_all, signal)
    }
}

impl<I, S, B, E> Future for Server<I, S, E>
where
    I: Stream,
    I::Error: Into<Box<::std::error::Error + Send + Sync>>,
    I::Item: AsyncRead + AsyncWrite + Send + 'static,
    S: NewService<ReqBody=Body, ResBody=B>,
    S::Error: Into<Box<::std::error::Error + Send + Sync>>,
    S::Service: 'static,
    B: Payload,
    E: H2Exec<<S::Service as Service>::Future, B>,
    E: NewSvcExec<I::Item, S::Future, S::Service, E, NoopWatcher>,
{
    type Item = ();
    type Error = ::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        self.spawn_all.poll_watch(&NoopWatcher)
    }
}

impl<I: fmt::Debug, S: fmt::Debug> fmt::Debug for Server<I, S> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Server")
            .field("listener", &self.spawn_all.incoming_ref())
            .finish()
    }
}

// ===== impl Builder =====

impl<I, E> Builder<I, E> {
    /// Start a new builder, wrapping an incoming stream and low-level options.
    ///
    /// For a more convenient constructor, see [`Server::bind`](Server::bind).
    pub fn new(incoming: I, protocol: Http_<E>) -> Self {
        Builder {
            incoming,
            protocol,
        }
    }

    /// Sets whether to use keep-alive for HTTP/1 connections.
    ///
    /// Default is `true`.
    pub fn http1_keepalive(mut self, val: bool) -> Self {
        self.protocol.keep_alive(val);
        self
    }

    /// Sets whether HTTP/1 is required.
    ///
    /// Default is `false`.
    pub fn http1_only(mut self, val: bool) -> Self {
        self.protocol.http1_only(val);
        self
    }

    // Sets whether to bunch up HTTP/1 writes until the read buffer is empty.
    //
    // This isn't really desirable in most cases, only really being useful in
    // silly pipeline benchmarks.
    #[doc(hidden)]
    pub fn http1_pipeline_flush(mut self, val: bool) -> Self {
        self.protocol.pipeline_flush(val);
        self
    }

    /// Set whether HTTP/1 connections should try to use vectored writes,
    /// or always flatten into a single buffer.
    ///
    /// # Note
    ///
    /// Setting this to `false` may mean more copies of body data,
    /// but may also improve performance when an IO transport doesn't
    /// support vectored writes well, such as most TLS implementations.
    ///
    /// Default is `true`.
    pub fn http1_writev(mut self, val: bool) -> Self {
        self.protocol.http1_writev(val);
        self
    }

    /// Sets whether HTTP/2 is required.
    ///
    /// Default is `false`.
    pub fn http2_only(mut self, val: bool) -> Self {
        self.protocol.http2_only(val);
        self
    }

    /// Sets the `Executor` to deal with connection tasks.
    ///
    /// Default is `tokio::spawn`.
    pub fn executor<E2>(self, executor: E2) -> Builder<I, E2> {
        Builder {
            incoming: self.incoming,
            protocol: self.protocol.with_executor(executor),
        }
    }

    /// Consume this `Builder`, creating a [`Server`](Server).
    ///
    /// # Example
    ///
    /// ```
    /// # extern crate hyper;
    /// # fn main() {}
    /// # #[cfg(feature = "runtime")]
    /// # fn run() {
    /// use hyper::{Body, Response, Server};
    /// use hyper::service::service_fn_ok;
    ///
    /// // Construct our SocketAddr to listen on...
    /// let addr = ([127, 0, 0, 1], 3000).into();
    ///
    /// // And a NewService to handle each connection...
    /// let new_service = || {
    ///     service_fn_ok(|_req| {
    ///         Response::new(Body::from("Hello World"))
    ///     })
    /// };
    ///
    /// // Then bind and serve...
    /// let server = Server::bind(&addr)
    ///     .serve(new_service);
    ///
    /// // Finally, spawn `server` onto an Executor...
    /// # }
    /// ```
    pub fn serve<S, B>(self, new_service: S) -> Server<I, S, E>
    where
        I: Stream,
        I::Error: Into<Box<::std::error::Error + Send + Sync>>,
        I::Item: AsyncRead + AsyncWrite + Send + 'static,
        S: NewService<ReqBody=Body, ResBody=B>,
        S::Error: Into<Box<::std::error::Error + Send + Sync>>,
        S::Service: 'static,
        B: Payload,
        E: NewSvcExec<I::Item, S::Future, S::Service, E, NoopWatcher>,
        E: H2Exec<<S::Service as Service>::Future, B>,
    {
        let serve = self.protocol.serve_incoming(self.incoming, new_service);
        let spawn_all = serve.spawn_all();
        Server {
            spawn_all,
        }
    }
}

#[cfg(feature = "runtime")]
impl<E> Builder<AddrIncoming, E> {
    /// Set whether TCP keepalive messages are enabled on accepted connections.
    ///
    /// If `None` is specified, keepalive is disabled, otherwise the duration
    /// specified will be the time to remain idle before sending TCP keepalive
    /// probes.
    pub fn tcp_keepalive(mut self, keepalive: Option<Duration>) -> Self {
        self.incoming.set_keepalive(keepalive);
        self
    }

    /// Set the value of `TCP_NODELAY` option for accepted connections.
    pub fn tcp_nodelay(mut self, enabled: bool) -> Self {
        self.incoming.set_nodelay(enabled);
        self
    }
}
