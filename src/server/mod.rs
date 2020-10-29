//! HTTP Server
//!
//! A `Server` is created to listen on a port, parse HTTP requests, and hand
//! them off to a `Service`.
//!
//! There are two levels of APIs provide for constructing HTTP servers:
//!
//! - The higher-level [`Server`](Server) type.
//! - The lower-level [`conn`](conn) module.
//!
//! # Server
//!
//! The [`Server`](Server) is main way to start listening for HTTP requests.
//! It wraps a listener with a [`MakeService`](crate::service), and then should
//! be executed to start serving requests.
//!
//! [`Server`](Server) accepts connections in both HTTP1 and HTTP2 by default.
//!
//! ## Example
//!
//! ```no_run
//! use std::convert::Infallible;
//! use std::net::SocketAddr;
//! use hyper::{Body, Request, Response, Server};
//! use hyper::service::{make_service_fn, service_fn};
//!
//! async fn handle(_req: Request<Body>) -> Result<Response<Body>, Infallible> {
//!     Ok(Response::new(Body::from("Hello World")))
//! }
//!
//! # #[cfg(feature = "runtime")]
//! #[tokio::main]
//! async fn main() {
//!     // Construct our SocketAddr to listen on...
//!     let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
//!
//!     // And a MakeService to handle each connection...
//!     let make_service = make_service_fn(|_conn| async {
//!         Ok::<_, Infallible>(service_fn(handle))
//!     });
//!
//!     // Then bind and serve...
//!     let server = Server::bind(&addr).serve(make_service);
//!
//!     // And run forever...
//!     if let Err(e) = server.await {
//!         eprintln!("server error: {}", e);
//!     }
//! }
//! # #[cfg(not(feature = "runtime"))]
//! # fn main() {}
//! ```

pub(crate) mod conn;
#[cfg(feature = "tcp")]
mod tcp;

pub use conn::{Connection, Parts};
#[cfg(feature = "tcp")]
pub use tcp::{AddrIncoming, AddrStream};

use std::error::Error as StdError;

use tokio::io::{AsyncRead, AsyncWrite};

use crate::common::exec::{Exec, H2Exec};
use crate::common::io::Rewind;
use crate::common::Future;
use crate::error::Error;
use crate::proto;
use crate::service::{util::ServiceFn, HttpService};
use crate::{
    body::{Body, HttpBody},
    common::exec::SvcExec,
};
use crate::{Request, Response};

#[cfg(feature = "tcp")]
use std::net::SocketAddr;
#[cfg(feature = "runtime")]
use std::time::Duration;

use conn::{Fallback, ProtoServer};

/// A lower-level configuration of the HTTP protocol.
///
/// This structure is used to configure options for an HTTP server connection.
///
/// If you don't have need to manage connections yourself, consider using the
/// higher-level [Server](super) API.
#[derive(Clone, Debug)]
pub struct Server<E = Exec> {
    exec: E,
    h1_half_close: bool,
    h1_keep_alive: bool,
    h1_writev: Option<bool>,
    h2_builder: proto::h2::server::Config,
    mode: ConnectionMode,
    max_buf_size: Option<usize>,
    pipeline_flush: bool,
}

/// The internal mode of HTTP protocol which indicates the behavior when a parse error occurs.
#[derive(Clone, Debug, PartialEq)]
enum ConnectionMode {
    /// Always use HTTP/1 and do not upgrade when a parse error occurs.
    H1Only,
    /// Always use HTTP/2.
    H2Only,
    /// Use HTTP/1 and try to upgrade to h2 when a parse error occurs.
    Fallback,
}

/// DOX
#[derive(Debug)]
pub struct Serve<E = Exec> {
    server: Server<E>,
    incoming: AddrIncoming,
}

// ===== impl Server =====

impl Server {
    /// Creates a new instance of the HTTP protocol, ready to spawn a server or
    /// start accepting connections.
    pub fn new() -> Server {
        Server {
            exec: Exec::Default,
            h1_half_close: false,
            h1_keep_alive: true,
            h1_writev: None,
            h2_builder: Default::default(),
            mode: ConnectionMode::Fallback,
            max_buf_size: None,
            pipeline_flush: false,
        }
    }
}

impl<E> Server<E> {
    /// Sets whether HTTP1 is required.
    ///
    /// Default is false
    pub fn http1_only(&mut self, val: bool) -> &mut Self {
        if val {
            self.mode = ConnectionMode::H1Only;
        } else {
            self.mode = ConnectionMode::Fallback;
        }
        self
    }

    /// Set whether HTTP/1 connections should support half-closures.
    ///
    /// Clients can chose to shutdown their write-side while waiting
    /// for the server to respond. Setting this to `true` will
    /// prevent closing the connection immediately if `read`
    /// detects an EOF in the middle of a request.
    ///
    /// Default is `false`.
    pub fn http1_half_close(&mut self, val: bool) -> &mut Self {
        self.h1_half_close = val;
        self
    }

    /// Enables or disables HTTP/1 keep-alive.
    ///
    /// Default is true.
    pub fn http1_keep_alive(&mut self, val: bool) -> &mut Self {
        self.h1_keep_alive = val;
        self
    }

    // renamed due different semantics of http2 keep alive
    #[doc(hidden)]
    #[deprecated(note = "renamed to `http1_keep_alive`")]
    pub fn keep_alive(&mut self, val: bool) -> &mut Self {
        self.http1_keep_alive(val)
    }

    /// Set whether HTTP/1 connections should try to use vectored writes,
    /// or always flatten into a single buffer.
    ///
    /// Note that setting this to false may mean more copies of body data,
    /// but may also improve performance when an IO transport doesn't
    /// support vectored writes well, such as most TLS implementations.
    ///
    /// Setting this to true will force hyper to use queued strategy
    /// which may eliminate unnecessary cloning on some TLS backends
    ///
    /// Default is `auto`. In this mode hyper will try to guess which
    /// mode to use
    #[inline]
    pub fn http1_writev(&mut self, val: bool) -> &mut Self {
        self.h1_writev = Some(val);
        self
    }

    /// Sets whether HTTP2 is required.
    ///
    /// Default is false
    pub fn http2_only(&mut self, val: bool) -> &mut Self {
        if val {
            self.mode = ConnectionMode::H2Only;
        } else {
            self.mode = ConnectionMode::Fallback;
        }
        self
    }

    /// Sets the [`SETTINGS_INITIAL_WINDOW_SIZE`][spec] option for HTTP2
    /// stream-level flow control.
    ///
    /// Passing `None` will do nothing.
    ///
    /// If not set, hyper will use a default.
    ///
    /// [spec]: https://http2.github.io/http2-spec/#SETTINGS_INITIAL_WINDOW_SIZE
    pub fn http2_initial_stream_window_size(&mut self, sz: impl Into<Option<u32>>) -> &mut Self {
        if let Some(sz) = sz.into() {
            self.h2_builder.adaptive_window = false;
            self.h2_builder.initial_stream_window_size = sz;
        }
        self
    }

    /// Sets the max connection-level flow control for HTTP2.
    ///
    /// Passing `None` will do nothing.
    ///
    /// If not set, hyper will use a default.
    pub fn http2_initial_connection_window_size(
        &mut self,
        sz: impl Into<Option<u32>>,
    ) -> &mut Self {
        if let Some(sz) = sz.into() {
            self.h2_builder.adaptive_window = false;
            self.h2_builder.initial_conn_window_size = sz;
        }
        self
    }

    /// Sets whether to use an adaptive flow control.
    ///
    /// Enabling this will override the limits set in
    /// `http2_initial_stream_window_size` and
    /// `http2_initial_connection_window_size`.
    pub fn http2_adaptive_window(&mut self, enabled: bool) -> &mut Self {
        use proto::h2::SPEC_WINDOW_SIZE;

        self.h2_builder.adaptive_window = enabled;
        if enabled {
            self.h2_builder.initial_conn_window_size = SPEC_WINDOW_SIZE;
            self.h2_builder.initial_stream_window_size = SPEC_WINDOW_SIZE;
        }
        self
    }

    /// Sets the maximum frame size to use for HTTP2.
    ///
    /// Passing `None` will do nothing.
    ///
    /// If not set, hyper will use a default.
    pub fn http2_max_frame_size(&mut self, sz: impl Into<Option<u32>>) -> &mut Self {
        if let Some(sz) = sz.into() {
            self.h2_builder.max_frame_size = sz;
        }
        self
    }

    /// Sets the [`SETTINGS_MAX_CONCURRENT_STREAMS`][spec] option for HTTP2
    /// connections.
    ///
    /// Default is no limit (`std::u32::MAX`). Passing `None` will do nothing.
    ///
    /// [spec]: https://http2.github.io/http2-spec/#SETTINGS_MAX_CONCURRENT_STREAMS
    pub fn http2_max_concurrent_streams(&mut self, max: impl Into<Option<u32>>) -> &mut Self {
        self.h2_builder.max_concurrent_streams = max.into();
        self
    }

    /// Sets an interval for HTTP2 Ping frames should be sent to keep a
    /// connection alive.
    ///
    /// Pass `None` to disable HTTP2 keep-alive.
    ///
    /// Default is currently disabled.
    ///
    /// # Cargo Feature
    ///
    /// Requires the `runtime` cargo feature to be enabled.
    #[cfg(feature = "runtime")]
    pub fn http2_keep_alive_interval(
        &mut self,
        interval: impl Into<Option<Duration>>,
    ) -> &mut Self {
        self.h2_builder.keep_alive_interval = interval.into();
        self
    }

    /// Sets a timeout for receiving an acknowledgement of the keep-alive ping.
    ///
    /// If the ping is not acknowledged within the timeout, the connection will
    /// be closed. Does nothing if `http2_keep_alive_interval` is disabled.
    ///
    /// Default is 20 seconds.
    ///
    /// # Cargo Feature
    ///
    /// Requires the `runtime` cargo feature to be enabled.
    #[cfg(feature = "runtime")]
    pub fn http2_keep_alive_timeout(&mut self, timeout: Duration) -> &mut Self {
        self.h2_builder.keep_alive_timeout = timeout;
        self
    }

    /// Set the maximum buffer size for the connection.
    ///
    /// Default is ~400kb.
    ///
    /// # Panics
    ///
    /// The minimum value allowed is 8192. This method panics if the passed `max` is less than the minimum.
    pub fn max_buf_size(&mut self, max: usize) -> &mut Self {
        assert!(
            max >= proto::h1::MINIMUM_MAX_BUFFER_SIZE,
            "the max_buf_size cannot be smaller than the minimum that h1 specifies."
        );
        self.max_buf_size = Some(max);
        self
    }

    /// Aggregates flushes to better support pipelined responses.
    ///
    /// Experimental, may have bugs.
    ///
    /// Default is false.
    pub fn pipeline_flush(&mut self, enabled: bool) -> &mut Self {
        self.pipeline_flush = enabled;
        self
    }

    /// Set the executor used to spawn background tasks.
    ///
    /// Default uses implicit default (like `tokio::spawn`).
    pub fn with_executor<E2>(self, exec: E2) -> Server<E2> {
        Server {
            exec,
            h1_half_close: self.h1_half_close,
            h1_keep_alive: self.h1_keep_alive,
            h1_writev: self.h1_writev,
            h2_builder: self.h2_builder,
            mode: self.mode,
            max_buf_size: self.max_buf_size,
            pipeline_flush: self.pipeline_flush,
        }
    }

    /// DOX
    #[cfg(feature = "tcp")]
    pub fn bind(&self, addr: &SocketAddr) -> Result<Serve<E>, Error>
    where
        E: Clone,
    {
        let incoming = AddrIncoming::bind(addr).map_err(Error::new_listen)?;
        let server = self.clone();

        Ok(Serve { server, incoming })
    }

    /// Bind a connection together with a [`Service`](crate::service::Service).
    ///
    /// This returns a Future that must be polled in order for HTTP to be
    /// driven on the connection.
    ///
    /// # Example
    ///
    /// ```
    /// # use hyper::{Body, Request, Response};
    /// # use hyper::service::Service;
    /// # use hyper::server::conn::Http;
    /// # use tokio::io::{AsyncRead, AsyncWrite};
    /// # async fn run<I, S>(some_io: I, some_service: S)
    /// # where
    /// #     I: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    /// #     S: Service<hyper::Request<Body>, Response=hyper::Response<Body>> + Send + 'static,
    /// #     S::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
    /// #     S::Future: Send,
    /// # {
    /// let http = Server::new();
    /// let conn = http.serve_connection(some_io, some_service);
    ///
    /// if let Err(e) = conn.await {
    ///     eprintln!("server connection error: {}", e);
    /// }
    /// # }
    /// # fn main() {}
    /// ```
    pub fn serve_connection<S, I, Bd>(&self, io: I, service: S) -> Connection<I, S, E>
    where
        S: HttpService<Body, ResBody = Bd>,
        S::Error: Into<Box<dyn StdError + Send + Sync>>,
        Bd: HttpBody + 'static,
        Bd::Error: Into<Box<dyn StdError + Send + Sync>>,
        I: AsyncRead + AsyncWrite + Unpin,
        E: H2Exec<S::Future, Bd>,
    {
        let proto = match self.mode {
            ConnectionMode::H1Only | ConnectionMode::Fallback => {
                let mut conn = proto::Conn::new(io);
                if !self.h1_keep_alive {
                    conn.disable_keep_alive();
                }
                if self.h1_half_close {
                    conn.set_allow_half_close();
                }
                if let Some(writev) = self.h1_writev {
                    if writev {
                        conn.set_write_strategy_queue();
                    } else {
                        conn.set_write_strategy_flatten();
                    }
                }
                conn.set_flush_pipeline(self.pipeline_flush);
                if let Some(max) = self.max_buf_size {
                    conn.set_max_buf_size(max);
                }
                let sd = proto::h1::dispatch::Server::new(service);
                ProtoServer::H1(proto::h1::Dispatcher::new(sd, conn))
            }
            ConnectionMode::H2Only => {
                let rewind_io = Rewind::new(io);
                let h2 =
                    proto::h2::Server::new(rewind_io, service, &self.h2_builder, self.exec.clone());
                ProtoServer::H2(h2)
            }
        };

        Connection {
            conn: Some(proto),
            fallback: if self.mode == ConnectionMode::Fallback {
                Fallback::ToHttp2(self.h2_builder.clone(), self.exec.clone())
            } else {
                Fallback::Http1Only
            },
        }
    }
}

// ===== impl Serve =====

impl<E> Serve<E> {
    /// DOX
    pub fn local_addr(&self) -> SocketAddr {
        self.incoming.local_addr()
    }

    /// DOX!
    #[cfg(all(feature = "tcp", feature = "runtime"))]
    pub async fn serve_fn<F, R, Bd, Er>(&mut self, f: F) -> Result<(), Error>
    where
        F: FnMut(Request<Body>) -> R + Clone,
        R: Future<Output = Result<Response<Bd>, Er>>,
        Er: Into<Box<dyn StdError + Send + Sync>>,
        Bd: HttpBody + 'static,
        Bd::Error: Into<Box<dyn StdError + Send + Sync>>,
        E: SvcExec<AddrStream, ServiceFn<F, Body>, E>,
        E: H2Exec<R, Bd>,
    {
        let svc = crate::service::service_fn(f);
        self.serve(svc).await
    }

    /// DOX
    #[cfg(all(feature = "tcp", feature = "runtime"))]
    pub async fn serve<S, Bd>(&mut self, service: S) -> Result<(), Error>
    where
        S: HttpService<Body, ResBody = Bd> + Clone,
        S::Error: Into<Box<dyn StdError + Send + Sync>>,
        Bd: HttpBody + 'static,
        Bd::Error: Into<Box<dyn StdError + Send + Sync>>,
        E: SvcExec<AddrStream, S, E>,
        E: H2Exec<S::Future, Bd>,
    {
        loop {
            let conn = self.incoming.accept().await.map_err(Error::new_accept)?;

            let fut = self.server.serve_connection(conn, service.clone());
            let svc_task = conn::spawn_all::SvcTask::new(fut);
            self.server.exec.execute_svc(svc_task);
        }
    }
}
