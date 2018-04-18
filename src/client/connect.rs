//! The `Connect` trait, and supporting types.
//!
//! This module contains:
//!
//! - A default [`HttpConnector`](HttpConnector) that does DNS resolution and
//!   establishes connections over TCP.
//! - The [`Connect`](Connect) trait and related types to build custom connectors.
use std::error::Error as StdError;
use std::fmt;
use std::io;
use std::mem;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use futures::{Future, Poll, Async};
use futures::future::{Executor, ExecuteError};
use futures::sync::oneshot;
use futures_cpupool::{Builder as CpuPoolBuilder};
use http::Uri;
use http::uri::Scheme;
use net2::TcpBuilder;
use tokio_io::{AsyncRead, AsyncWrite};
use tokio::reactor::Handle;
use tokio::net::{TcpStream, ConnectFuture};

use super::dns;
use self::http_connector::HttpConnectorBlockingTask;

/// Connect to a destination, returning an IO transport.
///
/// A connector receives a [`Destination`](Destination) describing how a
/// connection should be estabilished, and returns a `Future` of the
/// ready connection.
pub trait Connect: Send + Sync {
    /// The connected IO Stream.
    type Transport: AsyncRead + AsyncWrite + Send + 'static;
    /// An error occured when trying to connect.
    type Error: Into<Box<StdError + Send + Sync>>;
    /// A Future that will resolve to the connected Transport.
    type Future: Future<Item=(Self::Transport, Connected), Error=Self::Error> + Send;
    /// Connect to a destination.
    fn connect(&self, dst: Destination) -> Self::Future;
}

/// A set of properties to describe where and how to try to connect.
#[derive(Debug)]
pub struct Destination {
    //pub(super) alpn: Alpn,
    pub(super) uri: Uri,
}

/// Extra information about the connected transport.
///
/// This can be used to inform recipients about things like if ALPN
/// was used, or if connected to an HTTP proxy.
#[derive(Debug)]
pub struct Connected {
    //alpn: Alpn,
    pub(super) is_proxied: bool,
}

/*TODO: when HTTP1 Upgrades to H2 are added, this will be needed
#[derive(Debug)]
pub(super) enum Alpn {
    Http1,
    //H2,
    //Http1OrH2
}
*/

impl Destination {
    /// Get the protocol scheme.
    #[inline]
    pub fn scheme(&self) -> &str {
        self.uri
            .scheme_part()
            .expect("destination uri has scheme")
            .as_str()
    }

    /// Get the hostname.
    #[inline]
    pub fn host(&self) -> &str {
        self.uri
            .host()
            .expect("destination uri has host")
    }

    /// Get the port, if specified.
    #[inline]
    pub fn port(&self) -> Option<u16> {
        self.uri.port()
    }

    /*
    /// Returns whether this connection must negotiate HTTP/2 via ALPN.
    pub fn must_h2(&self) -> bool {
        match self.alpn {
            Alpn::Http1 => false,
            Alpn::H2 => true,
        }
    }
    */
}

impl Connected {
    /// Create new `Connected` type with empty metadata.
    pub fn new() -> Connected {
        Connected {
            //alpn: Alpn::Http1,
            is_proxied: false,
        }
    }

    /// Set whether the connected transport is to an HTTP proxy.
    ///
    /// This setting will affect if HTTP/1 requests written on the transport
    /// will have the request-target in absolute-form or origin-form (such as
    /// `GET http://hyper.rs/guide HTTP/1.1` or `GET /guide HTTP/1.1`).
    ///
    /// Default is `false`.
    pub fn proxy(mut self, is_proxied: bool) -> Connected {
        self.is_proxied = is_proxied;
        self
    }

    /*
    /// Set that the connected transport negotiated HTTP/2 as it's
    /// next protocol.
    pub fn h2(mut self) -> Connected {
        self.alpn = Alpn::H2;
        self
    }
    */
}

fn connect(addr: &SocketAddr, handle: &Option<Handle>) -> io::Result<ConnectFuture> {
    if let Some(ref handle) = *handle {
        let builder = match addr {
            &SocketAddr::V4(_) => TcpBuilder::new_v4()?,
            &SocketAddr::V6(_) => TcpBuilder::new_v6()?,
        };

        if cfg!(windows) {
            // Windows requires a socket be bound before calling connect
            let any: SocketAddr = match addr {
                &SocketAddr::V4(_) => {
                    ([0, 0, 0, 0], 0).into()
                },
                &SocketAddr::V6(_) => {
                    ([0, 0, 0, 0, 0, 0, 0, 0], 0).into()
                }
            };
            builder.bind(any)?;
        }

        Ok(TcpStream::connect_std(builder.to_tcp_stream()?, addr, handle))
    } else {
        Ok(TcpStream::connect(addr))
    }
}

/// A connector for the `http` scheme.
///
/// Performs DNS resolution in a thread pool, and then connects over TCP.
#[derive(Clone)]
pub struct HttpConnector {
    executor: HttpConnectExecutor,
    enforce_http: bool,
    handle: Option<Handle>,
    keep_alive_timeout: Option<Duration>,
    nodelay: bool,
}

impl HttpConnector {
    /// Construct a new HttpConnector.
    ///
    /// Takes number of DNS worker threads.
    #[inline]
    pub fn new(threads: usize) -> HttpConnector {
        HttpConnector::new_with_handle_opt(threads, None)
    }

    /// Construct a new HttpConnector with a specific Tokio handle.
    pub fn new_with_handle(threads: usize, handle: Handle) -> HttpConnector {
        HttpConnector::new_with_handle_opt(threads, Some(handle))
    }

    fn new_with_handle_opt(threads: usize, handle: Option<Handle>) -> HttpConnector {
        let pool = CpuPoolBuilder::new()
            .name_prefix("hyper-dns")
            .pool_size(threads)
            .create();
        HttpConnector::new_with_executor(pool, handle)
    }

    /// Construct a new HttpConnector.
    ///
    /// Takes an executor to run blocking tasks on.
    pub fn new_with_executor<E: 'static>(executor: E, handle: Option<Handle>) -> HttpConnector
        where E: Executor<HttpConnectorBlockingTask> + Send + Sync
    {
        HttpConnector {
            executor: HttpConnectExecutor(Arc::new(executor)),
            enforce_http: true,
            handle,
            keep_alive_timeout: None,
            nodelay: false,
        }
    }

    /// Option to enforce all `Uri`s have the `http` scheme.
    ///
    /// Enabled by default.
    #[inline]
    pub fn enforce_http(&mut self, is_enforced: bool) {
        self.enforce_http = is_enforced;
    }

    /// Set that all sockets have `SO_KEEPALIVE` set with the supplied duration.
    ///
    /// If `None`, the option will not be set.
    ///
    /// Default is `None`.
    #[inline]
    pub fn set_keepalive(&mut self, dur: Option<Duration>) {
        self.keep_alive_timeout = dur;
    }

    /// Set that all sockets have `SO_NODELAY` set to the supplied value `nodelay`.
    ///
    /// Default is `false`.
    #[inline]
    pub fn set_nodelay(&mut self, nodelay: bool) {
        self.nodelay = nodelay;
    }
}

impl fmt::Debug for HttpConnector {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("HttpConnector")
            .finish()
    }
}

impl Connect for HttpConnector {
    type Transport = TcpStream;
    type Error = io::Error;
    type Future = HttpConnecting;

    fn connect(&self, dst: Destination) -> Self::Future {
        trace!(
            "Http::connect; scheme={}, host={}, port={:?}",
            dst.scheme(),
            dst.host(),
            dst.port(),
        );

        if self.enforce_http {
            if dst.uri.scheme_part() != Some(&Scheme::HTTP) {
                return invalid_url(InvalidUrl::NotHttp, &self.handle);
            }
        } else if dst.uri.scheme_part().is_none() {
            return invalid_url(InvalidUrl::MissingScheme, &self.handle);
        }

        let host = match dst.uri.host() {
            Some(s) => s,
            None => return invalid_url(InvalidUrl::MissingAuthority, &self.handle),
        };
        let port = match dst.uri.port() {
            Some(port) => port,
            None => if dst.uri.scheme_part() == Some(&Scheme::HTTPS) { 443 } else { 80 },
        };

        HttpConnecting {
            state: State::Lazy(self.executor.clone(), host.into(), port),
            handle: self.handle.clone(),
            keep_alive_timeout: self.keep_alive_timeout,
            nodelay: self.nodelay,
        }
    }
}

#[inline]
fn invalid_url(err: InvalidUrl, handle: &Option<Handle>) -> HttpConnecting {
    HttpConnecting {
        state: State::Error(Some(io::Error::new(io::ErrorKind::InvalidInput, err))),
        handle: handle.clone(),
        keep_alive_timeout: None,
        nodelay: false,
    }
}

#[derive(Debug, Clone, Copy)]
enum InvalidUrl {
    MissingScheme,
    NotHttp,
    MissingAuthority,
}

impl fmt::Display for InvalidUrl {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.description())
    }
}

impl StdError for InvalidUrl {
    fn description(&self) -> &str {
        match *self {
            InvalidUrl::MissingScheme => "invalid URL, missing scheme",
            InvalidUrl::NotHttp => "invalid URL, scheme must be http",
            InvalidUrl::MissingAuthority => "invalid URL, missing domain",
        }
    }
}

/// A Future representing work to connect to a URL.
#[must_use = "futures do nothing unless polled"]
pub struct HttpConnecting {
    state: State,
    handle: Option<Handle>,
    keep_alive_timeout: Option<Duration>,
    nodelay: bool,
}

enum State {
    Lazy(HttpConnectExecutor, String, u16),
    Resolving(oneshot::SpawnHandle<dns::IpAddrs, io::Error>),
    Connecting(ConnectingTcp),
    Error(Option<io::Error>),
}

impl Future for HttpConnecting {
    type Item = (TcpStream, Connected);
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        loop {
            let state;
            match self.state {
                State::Lazy(ref executor, ref mut host, port) => {
                    // If the host is already an IP addr (v4 or v6),
                    // skip resolving the dns and start connecting right away.
                    if let Some(addrs) = dns::IpAddrs::try_parse(host, port) {
                        state = State::Connecting(ConnectingTcp {
                            addrs: addrs,
                            current: None
                        })
                    } else {
                        let host = mem::replace(host, String::new());
                        let work = dns::Work::new(host, port);
                        state = State::Resolving(oneshot::spawn(work, executor));
                    }
                },
                State::Resolving(ref mut future) => {
                    match try!(future.poll()) {
                        Async::NotReady => return Ok(Async::NotReady),
                        Async::Ready(addrs) => {
                            state = State::Connecting(ConnectingTcp {
                                addrs: addrs,
                                current: None,
                            })
                        }
                    };
                },
                State::Connecting(ref mut c) => {
                    let sock = try_ready!(c.poll(&self.handle));

                    if let Some(dur) = self.keep_alive_timeout {
                        sock.set_keepalive(Some(dur))?;
                    }

                    sock.set_nodelay(self.nodelay)?;

                    return Ok(Async::Ready((sock, Connected::new())));
                },
                State::Error(ref mut e) => return Err(e.take().expect("polled more than once")),
            }
            self.state = state;
        }
    }
}

impl fmt::Debug for HttpConnecting {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad("HttpConnecting")
    }
}

struct ConnectingTcp {
    addrs: dns::IpAddrs,
    current: Option<ConnectFuture>,
}

impl ConnectingTcp {
    // not a Future, since passing a &Handle to poll
    fn poll(&mut self, handle: &Option<Handle>) -> Poll<TcpStream, io::Error> {
        let mut err = None;
        loop {
            if let Some(ref mut current) = self.current {
                match current.poll() {
                    Ok(ok) => return Ok(ok),
                    Err(e) => {
                        trace!("connect error {:?}", e);
                        err = Some(e);
                        if let Some(addr) = self.addrs.next() {
                            debug!("connecting to {}", addr);
                            *current = connect(&addr, handle)?;
                            continue;
                        }
                    }
                }
            } else if let Some(addr) = self.addrs.next() {
                debug!("connecting to {}", addr);
                self.current = Some(connect(&addr, handle)?);
                continue;
            }

            return Err(err.take().expect("missing connect error"));
        }
    }
}

// Make this Future unnameable outside of this crate.
mod http_connector {
    use super::*;
    // Blocking task to be executed on a thread pool.
    pub struct HttpConnectorBlockingTask {
        pub(super) work: oneshot::Execute<dns::Work>
    }

    impl fmt::Debug for HttpConnectorBlockingTask {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.pad("HttpConnectorBlockingTask")
        }
    }

    impl Future for HttpConnectorBlockingTask {
        type Item = ();
        type Error = ();

        fn poll(&mut self) -> Poll<(), ()> {
            self.work.poll()
        }
    }
}

#[derive(Clone)]
struct HttpConnectExecutor(Arc<Executor<HttpConnectorBlockingTask> + Send + Sync>);

impl Executor<oneshot::Execute<dns::Work>> for HttpConnectExecutor {
    fn execute(&self, future: oneshot::Execute<dns::Work>) -> Result<(), ExecuteError<oneshot::Execute<dns::Work>>> {
        self.0.execute(HttpConnectorBlockingTask { work: future })
            .map_err(|err| ExecuteError::new(err.kind(), err.into_future().work))
    }
}

#[cfg(test)]
mod tests {
    #![allow(deprecated)]
    use std::io;
    use futures::Future;
    use super::{Connect, Destination, HttpConnector};

    #[test]
    fn test_errors_missing_authority() {
        let uri = "/foo/bar?baz".parse().unwrap();
        let dst = Destination {
            uri,
        };
        let connector = HttpConnector::new(1);

        assert_eq!(connector.connect(dst).wait().unwrap_err().kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn test_errors_enforce_http() {
        let uri = "https://example.domain/foo/bar?baz".parse().unwrap();
        let dst = Destination {
            uri,
        };
        let connector = HttpConnector::new(1);

        assert_eq!(connector.connect(dst).wait().unwrap_err().kind(), io::ErrorKind::InvalidInput);
    }


    #[test]
    fn test_errors_missing_scheme() {
        let uri = "example.domain".parse().unwrap();
        let dst = Destination {
            uri,
        };
        let connector = HttpConnector::new(1);

        assert_eq!(connector.connect(dst).wait().unwrap_err().kind(), io::ErrorKind::InvalidInput);
    }
}
