//! HTTP Server
//!
//! A `Server` is created to listen on a port, parse HTTP requests, and hand
//! them off to a `Service`.
//!
//! There are two levels of APIs provide for constructing HTTP servers:
//!
//! - The higher-level [`Server`](Server).
//! - The lower-level [conn](conn) module.

pub mod conn;
mod service;
mod tcp;

use std::fmt;
use std::net::SocketAddr;
use std::time::Duration;

use futures::{Future, Stream, Poll};
use http::{Request, Response};
use tokio_io::{AsyncRead, AsyncWrite};
pub use tokio_service::{NewService, Service};

use body::{Body, Payload};
// Renamed `Http` as `Http_` for now so that people upgrading don't see an
// error that `hyper::server::Http` is private...
use self::conn::{Http as Http_, SpawnAll};
use self::hyper_service::HyperService;
use self::tcp::{AddrIncoming};

pub use self::service::{const_service, service_fn};

/// A listening HTTP server.
///
/// `Server` is a `Future` mapping a bound listener with a set of service
/// handlers. It is built using the [`Builder`](Builder), and the future
/// completes when the server has been shutdown. It should be run by an
/// `Executor`.
pub struct Server<I, S> {
    spawn_all: SpawnAll<I, S>,
}

/// A builder for a [`Server`](Server).
#[derive(Debug)]
pub struct Builder<I> {
    incoming: I,
    protocol: Http_,
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
}

impl<S> Server<AddrIncoming, S> {
    /// Returns the local address that this server is bound to.
    pub fn local_addr(&self) -> SocketAddr {
        self.spawn_all.local_addr()
    }
}

impl<I, S, B> Future for Server<I, S>
where
    I: Stream,
    I::Error: Into<Box<::std::error::Error + Send + Sync>>,
    I::Item: AsyncRead + AsyncWrite + Send + 'static,
    S: NewService<Request = Request<Body>, Response = Response<B>> + Send + 'static,
    S::Error: Into<Box<::std::error::Error + Send + Sync>>,
    <S as NewService>::Instance: Send,
    <<S as NewService>::Instance as Service>::Future: Send + 'static,
    B: Payload,
{
    type Item = ();
    type Error = ::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        self.spawn_all.poll()
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

impl<I> Builder<I> {
    /// Start a new builder, wrapping an incoming stream and low-level options.
    ///
    /// For a more convenient constructor, see [`Server::bind`](Server::bind).
    pub fn new(incoming: I, protocol: Http_) -> Self {
        Builder {
            incoming,
            protocol,
        }
    }

    /// Sets whether HTTP/2 is required.
    ///
    /// Default is `false`.
    pub fn http2_only(mut self, val: bool) -> Self {
        self.protocol.http2_only(val);
        self
    }

    /// Consume this `Builder`, creating a [`Server`](Server).
    pub fn serve<S, B>(self, new_service: S) -> Server<I, S>
    where
        I: Stream,
        I::Error: Into<Box<::std::error::Error + Send + Sync>>,
        I::Item: AsyncRead + AsyncWrite + Send + 'static,
        S: NewService<Request = Request<Body>, Response = Response<B>>,
        S::Error: Into<Box<::std::error::Error + Send + Sync>>,
        <S as NewService>::Instance: Send,
        <<S as NewService>::Instance as Service>::Future: Send + 'static,
        B: Payload,
    {
        let serve = self.protocol.serve_incoming(self.incoming, new_service);
        let spawn_all = serve.spawn_all();
        Server {
            spawn_all,
        }
    }
}

impl Builder<AddrIncoming> {
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

mod hyper_service {
    use super::{Body, Payload, Request, Response, Service};
    /// A "trait alias" for any type that implements `Service` with hyper's
    /// Request, Response, and Error types, and a streaming body.
    ///
    /// There is an auto implementation inside hyper, so no one can actually
    /// implement this trait. It simply exists to reduce the amount of generics
    /// needed.
    pub trait HyperService: Service + Sealed {
        #[doc(hidden)]
        type ResponseBody;
        #[doc(hidden)]
        type Sealed: Sealed2;
    }

    pub trait Sealed {}
    pub trait Sealed2 {}

    #[allow(missing_debug_implementations)]
    pub struct Opaque {
        _inner: (),
    }

    impl Sealed2 for Opaque {}

    impl<S, B> Sealed for S
    where
        S: Service<
            Request=Request<Body>,
            Response=Response<B>,
        >,
        S::Error: Into<Box<::std::error::Error + Send + Sync>>,
        B: Payload,
    {}

    impl<S, B> HyperService for S
    where
        S: Service<
            Request=Request<Body>,
            Response=Response<B>,
        >,
        S::Error: Into<Box<::std::error::Error + Send + Sync>>,
        S: Sealed,
        B: Payload,
    {
        type ResponseBody = B;
        type Sealed = Opaque;
    }
}
