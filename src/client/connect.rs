//! The `Connect` trait, and supporting types.
//!
//! This module contains:
//!
//! - A default [`HttpConnector`](HttpConnector) that does DNS resolution and
//!   establishes connections over TCP.
//! - The [`Connect`](Connect) trait and related types to build custom connectors.
use std::error::Error as StdError;
use std::mem;

use bytes::{BufMut, BytesMut};
use futures::Future;
use http::{uri, Uri};
use tokio_io::{AsyncRead, AsyncWrite};

#[cfg(feature = "runtime")] pub use self::http::HttpConnector;

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
#[derive(Clone, Debug)]
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
            .map(|s| s.as_str())
            .unwrap_or("")
    }

    /// Get the hostname.
    #[inline]
    pub fn host(&self) -> &str {
        self.uri
            .host()
            .unwrap_or("")
    }

    /// Get the port, if specified.
    #[inline]
    pub fn port(&self) -> Option<u16> {
        self.uri.port()
    }

    /// Update the scheme of this destination.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use hyper::client::connect::Destination;
    /// # fn with_dst(mut dst: Destination) {
    /// // let mut dst = some_destination...
    /// // Change from "http://"...
    /// assert_eq!(dst.scheme(), "http");
    ///
    /// // to "ws://"...
    /// dst.set_scheme("ws");
    /// assert_eq!(dst.scheme(), "ws");
    /// # }
    /// ```
    ///
    /// # Error
    ///
    /// Returns an error if the string is not a valid scheme.
    pub fn set_scheme(&mut self, scheme: &str) -> ::Result<()> {
        let scheme = scheme.parse().map_err(::error::Parse::from)?;
        self.update_uri(move |parts| {
            parts.scheme = Some(scheme);
        })
    }

    /// Update the host of this destination.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use hyper::client::connect::Destination;
    /// # fn with_dst(mut dst: Destination) {
    /// // let mut dst = some_destination...
    /// // Change from "hyper.rs"...
    /// assert_eq!(dst.host(), "hyper.rs");
    ///
    /// // to "some.proxy"...
    /// dst.set_host("some.proxy");
    /// assert_eq!(dst.host(), "some.proxy");
    /// # }
    /// ```
    ///
    /// # Error
    ///
    /// Returns an error if the string is not a valid hostname.
    pub fn set_host(&mut self, host: &str) -> ::Result<()> {
        if host.contains(&['@',':'][..]) {
            return Err(::error::Parse::Uri.into());
        }
        let auth = if let Some(port) = self.port() {
            format!("{}:{}", host, port).parse().map_err(::error::Parse::from)?
        } else {
            host.parse().map_err(::error::Parse::from)?
        };
        self.update_uri(move |parts| {
            parts.authority = Some(auth);
        })
    }

    /// Update the port of this destination.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use hyper::client::connect::Destination;
    /// # fn with_dst(mut dst: Destination) {
    /// // let mut dst = some_destination...
    /// // Change from "None"...
    /// assert_eq!(dst.port(), None);
    ///
    /// // to "4321"...
    /// dst.set_port(4321);
    /// assert_eq!(dst.port(), Some(4321));
    ///
    /// // Or remove the port...
    /// dst.set_port(None);
    /// assert_eq!(dst.port(), None);
    /// # }
    /// ```
    pub fn set_port<P>(&mut self, port: P)
    where
        P: Into<Option<u16>>,
    {
        self.set_port_opt(port.into());
    }

    fn set_port_opt(&mut self, port: Option<u16>) {
        use std::fmt::Write;

        let auth = if let Some(port) = port {
            let host = self.host();
            // Need space to copy the hostname, plus ':',
            // plus max 5 port digits...
            let cap = host.len() + 1 + 5;
            let mut buf = BytesMut::with_capacity(cap);
            buf.put_slice(host.as_bytes());
            buf.put_u8(b':');
            write!(buf, "{}", port)
                .expect("should have space for 5 digits");

            uri::Authority::from_shared(buf.freeze())
                .expect("valid host + :port should be valid authority")
        } else {
            self.host().parse()
                .expect("valid host without port should be valid authority")
        };

        self.update_uri(move |parts| {
            parts.authority = Some(auth);
        })
            .expect("valid uri should be valid with port");
    }

    fn update_uri<F>(&mut self, f: F) -> ::Result<()>
    where
        F: FnOnce(&mut uri::Parts)
    {
        // Need to store a default Uri while we modify the current one...
        let old_uri = mem::replace(&mut self.uri, Uri::default());
        // However, mutate a clone, so we can revert if there's an error...
        let mut parts: uri::Parts = old_uri.clone().into();

        f(&mut parts);

        match Uri::from_parts(parts) {
            Ok(uri) => {
                self.uri = uri;
                Ok(())
            },
            Err(err) => {
                self.uri = old_uri;
                Err(::error::Parse::from(err).into())
            },
        }
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

#[cfg(test)]
mod tests {
    use super::Destination;

    #[test]
    fn test_destination_set_scheme() {
        let mut dst = Destination {
            uri: "http://hyper.rs".parse().expect("initial parse"),
        };

        assert_eq!(dst.scheme(), "http");
        assert_eq!(dst.host(), "hyper.rs");

        dst.set_scheme("https").expect("set https");
        assert_eq!(dst.scheme(), "https");
        assert_eq!(dst.host(), "hyper.rs");

        dst.set_scheme("<im not a scheme//?>").unwrap_err();
        assert_eq!(dst.scheme(), "https", "error doesn't modify dst");
        assert_eq!(dst.host(), "hyper.rs", "error doesn't modify dst");
    }

    #[test]
    fn test_destination_set_host() {
        let mut dst = Destination {
            uri: "http://hyper.rs".parse().expect("initial parse"),
        };

        assert_eq!(dst.scheme(), "http");
        assert_eq!(dst.host(), "hyper.rs");
        assert_eq!(dst.port(), None);

        dst.set_host("seanmonstar.com").expect("set https");
        assert_eq!(dst.scheme(), "http");
        assert_eq!(dst.host(), "seanmonstar.com");
        assert_eq!(dst.port(), None);

        dst.set_host("/im-not a host! >:)").unwrap_err();
        assert_eq!(dst.scheme(), "http", "error doesn't modify dst");
        assert_eq!(dst.host(), "seanmonstar.com", "error doesn't modify dst");
        assert_eq!(dst.port(), None, "error doesn't modify dst");

        // Also test that an exist port is set correctly.
        let mut dst = Destination {
            uri: "http://hyper.rs:8080".parse().expect("initial parse 2"),
        };

        assert_eq!(dst.scheme(), "http");
        assert_eq!(dst.host(), "hyper.rs");
        assert_eq!(dst.port(), Some(8080));

        dst.set_host("seanmonstar.com").expect("set host");
        assert_eq!(dst.scheme(), "http");
        assert_eq!(dst.host(), "seanmonstar.com");
        assert_eq!(dst.port(), Some(8080));

        dst.set_host("/im-not a host! >:)").unwrap_err();
        assert_eq!(dst.scheme(), "http", "error doesn't modify dst");
        assert_eq!(dst.host(), "seanmonstar.com", "error doesn't modify dst");
        assert_eq!(dst.port(), Some(8080), "error doesn't modify dst");

        // Check port isn't snuck into `set_host`.
        dst.set_host("seanmonstar.com:3030").expect_err("set_host sneaky port");
        assert_eq!(dst.scheme(), "http", "error doesn't modify dst");
        assert_eq!(dst.host(), "seanmonstar.com", "error doesn't modify dst");
        assert_eq!(dst.port(), Some(8080), "error doesn't modify dst");

        // Check userinfo isn't snuck into `set_host`.
        dst.set_host("sean@nope").expect_err("set_host sneaky userinfo");
        assert_eq!(dst.scheme(), "http", "error doesn't modify dst");
        assert_eq!(dst.host(), "seanmonstar.com", "error doesn't modify dst");
        assert_eq!(dst.port(), Some(8080), "error doesn't modify dst");
    }

    #[test]
    fn test_destination_set_port() {
        let mut dst = Destination {
            uri: "http://hyper.rs".parse().expect("initial parse"),
        };

        assert_eq!(dst.scheme(), "http");
        assert_eq!(dst.host(), "hyper.rs");
        assert_eq!(dst.port(), None);

        dst.set_port(None);
        assert_eq!(dst.scheme(), "http");
        assert_eq!(dst.host(), "hyper.rs");
        assert_eq!(dst.port(), None);

        dst.set_port(8080);
        assert_eq!(dst.scheme(), "http");
        assert_eq!(dst.host(), "hyper.rs");
        assert_eq!(dst.port(), Some(8080));

        // Also test that an exist port is set correctly.
        let mut dst = Destination {
            uri: "http://hyper.rs:8080".parse().expect("initial parse 2"),
        };

        assert_eq!(dst.scheme(), "http");
        assert_eq!(dst.host(), "hyper.rs");
        assert_eq!(dst.port(), Some(8080));

        dst.set_port(3030);
        assert_eq!(dst.scheme(), "http");
        assert_eq!(dst.host(), "hyper.rs");
        assert_eq!(dst.port(), Some(3030));

        dst.set_port(None);
        assert_eq!(dst.scheme(), "http");
        assert_eq!(dst.host(), "hyper.rs");
        assert_eq!(dst.port(), None);
    }
}

#[cfg(feature = "runtime")]
mod http {
    use super::*;

    use std::borrow::Cow;
    use std::fmt;
    use std::io;
    use std::mem;
    use std::net::{IpAddr, SocketAddr};
    use std::sync::Arc;
    use std::time::Duration;

    use futures::{Async, Poll};
    use futures::future::{Executor, ExecuteError};
    use futures::sync::oneshot;
    use futures_cpupool::{Builder as CpuPoolBuilder};
    use http::uri::Scheme;
    use net2::TcpBuilder;
    use tokio_reactor::Handle;
    use tokio_tcp::{TcpStream, ConnectFuture};

    use super::super::dns;

    use self::http_connector::HttpConnectorBlockingTask;


    fn connect(addr: &SocketAddr, local_addr: &Option<IpAddr>, handle: &Option<Handle>) -> io::Result<ConnectFuture> {
        let builder = match addr {
            &SocketAddr::V4(_) => TcpBuilder::new_v4()?,
            &SocketAddr::V6(_) => TcpBuilder::new_v6()?,
        };

        if let Some(ref local_addr) = *local_addr {
            // Caller has requested this socket be bound before calling connect
            builder.bind(SocketAddr::new(local_addr.clone(), 0))?;
        }
        else if cfg!(windows) {
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

        let handle = match *handle {
            Some(ref handle) => Cow::Borrowed(handle),
            None => Cow::Owned(Handle::current()),
        };

        Ok(TcpStream::connect_std(builder.to_tcp_stream()?, addr, &handle))
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
        local_address: Option<IpAddr>,
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
                local_address: None,
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

        /// Set that all sockets are bound to the configured address before connection.
        ///
        /// If `None`, the sockets will not be bound.
        ///
        /// Default is `None`.
        #[inline]
        pub fn set_local_address(&mut self, addr: Option<IpAddr>) {
            self.local_address = addr;
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
                state: State::Lazy(self.executor.clone(), host.into(), port, self.local_address),
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
        Lazy(HttpConnectExecutor, String, u16, Option<IpAddr>),
        Resolving(oneshot::SpawnHandle<dns::IpAddrs, io::Error>, Option<IpAddr>),
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
                    State::Lazy(ref executor, ref mut host, port, local_addr) => {
                        // If the host is already an IP addr (v4 or v6),
                        // skip resolving the dns and start connecting right away.
                        if let Some(addrs) = dns::IpAddrs::try_parse(host, port) {
                            state = State::Connecting(ConnectingTcp {
                                addrs: addrs,
                                local_addr: local_addr,
                                current: None
                            })
                        } else {
                            let host = mem::replace(host, String::new());
                            let work = dns::Work::new(host, port);
                            state = State::Resolving(oneshot::spawn(work, executor), local_addr);
                        }
                    },
                    State::Resolving(ref mut future, local_addr) => {
                        match try!(future.poll()) {
                            Async::NotReady => return Ok(Async::NotReady),
                            Async::Ready(addrs) => {
                                state = State::Connecting(ConnectingTcp {
                                    addrs: addrs,
                                    local_addr: local_addr,
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
        local_addr: Option<IpAddr>,
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
                                *current = connect(&addr, &self.local_addr, handle)?;
                                continue;
                            }
                        }
                    }
                } else if let Some(addr) = self.addrs.next() {
                    debug!("connecting to {}", addr);
                    self.current = Some(connect(&addr, &self.local_addr, handle)?);
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
}

