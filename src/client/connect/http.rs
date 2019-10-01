use std::borrow::Cow;
use std::fmt;
use std::error::Error as StdError;
use std::io;
use std::mem;
use std::net::{IpAddr, SocketAddr};
use std::time::{Duration, Instant};

use futures::{Async, Future, Poll};
use futures::future::{Executor};
use http::uri::Scheme;
use net2::TcpBuilder;
use tokio_reactor::Handle;
use tokio_tcp::{TcpStream, ConnectFuture};
use tokio_timer::{Delay, Timeout};

use super::{Connect, Connected, Destination};
use super::dns::{self, GaiResolver, Resolve, TokioThreadpoolGaiResolver};

/// A connector for the `http` scheme.
///
/// Performs DNS resolution in a thread pool, and then connects over TCP.
///
/// # Note
///
/// Sets the [`HttpInfo`](HttpInfo) value on responses, which includes
/// transport information such as the remote socket address used.
#[derive(Clone)]
pub struct HttpConnector<R = GaiResolver> {
    enforce_http: bool,
    handle: Option<Handle>,
    resolve_timeout: Option<Duration>,
    connect_timeout: Option<Duration>,
    happy_eyeballs_timeout: Option<Duration>,
    keep_alive_timeout: Option<Duration>,
    local_address: Option<IpAddr>,
    nodelay: bool,
    resolver: R,
    reuse_address: bool,
    send_buffer_size: Option<usize>,
    recv_buffer_size: Option<usize>,
}

/// Extra information about the transport when an HttpConnector is used.
///
/// # Example
///
/// ```
/// use hyper::Uri;
/// use hyper::client::{Client, connect::HttpInfo};
/// use hyper::rt::Future;
///
/// let client = Client::new();
///
/// let fut = client.get(Uri::from_static("http://example.local"))
///     .inspect(|resp| {
///         resp
///             .extensions()
///             .get::<HttpInfo>()
///             .map(|info| {
///                 println!("remote addr = {}", info.remote_addr());
///             });
///     });
/// ```
///
/// # Note
///
/// If a different connector is used besides [`HttpConnector`](HttpConnector),
/// this value will not exist in the extensions. Consult that specific
/// connector to see what "extra" information it might provide to responses.
#[derive(Clone, Debug)]
pub struct HttpInfo {
    remote_addr: SocketAddr,
}

impl HttpConnector {
    /// Construct a new HttpConnector.
    ///
    /// Takes number of DNS worker threads.
    #[inline]
    pub fn new(threads: usize) -> HttpConnector {
        HttpConnector::new_with_resolver(GaiResolver::new(threads))
    }

    #[doc(hidden)]
    #[deprecated(note = "Use HttpConnector::set_reactor to set a reactor handle")]
    pub fn new_with_handle(threads: usize, handle: Handle) -> HttpConnector {
        let resolver = GaiResolver::new(threads);
        let mut http = HttpConnector::new_with_resolver(resolver);
        http.set_reactor(Some(handle));
        http
    }

    /// Construct a new HttpConnector.
    ///
    /// Takes an executor to run blocking `getaddrinfo` tasks on.
    pub fn new_with_executor<E: 'static>(executor: E, handle: Option<Handle>) -> HttpConnector
        where E: Executor<dns::sealed::GaiTask> + Send + Sync
    {
        let resolver = GaiResolver::new_with_executor(executor);
        let mut http = HttpConnector::new_with_resolver(resolver);
        http.set_reactor(handle);
        http
    }
}

impl HttpConnector<TokioThreadpoolGaiResolver> {
    /// Construct a new HttpConnector using the `TokioThreadpoolGaiResolver`.
    ///
    /// This resolver **requires** the threadpool runtime to be used.
    pub fn new_with_tokio_threadpool_resolver() -> Self {
        HttpConnector::new_with_resolver(TokioThreadpoolGaiResolver::new())
    }
}


impl<R> HttpConnector<R> {
    /// Construct a new HttpConnector.
    ///
    /// Takes a `Resolve` to handle DNS lookups.
    pub fn new_with_resolver(resolver: R) -> HttpConnector<R> {
        HttpConnector {
            enforce_http: true,
            handle: None,
            resolve_timeout: None,
            connect_timeout: None,
            happy_eyeballs_timeout: Some(Duration::from_millis(300)),
            keep_alive_timeout: None,
            local_address: None,
            nodelay: false,
            resolver,
            reuse_address: false,
            send_buffer_size: None,
            recv_buffer_size: None,
        }
    }

    /// Option to enforce all `Uri`s have the `http` scheme.
    ///
    /// Enabled by default.
    #[inline]
    pub fn enforce_http(&mut self, is_enforced: bool) {
        self.enforce_http = is_enforced;
    }

    /// Set a handle to a `Reactor` to register connections to.
    ///
    /// If `None`, the implicit default reactor will be used.
    #[inline]
    pub fn set_reactor(&mut self, handle: Option<Handle>) {
        self.handle = handle;
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

    /// Sets the value of the SO_SNDBUF option on the socket.
    #[inline]
    pub fn set_send_buffer_size(&mut self, size: Option<usize>) {
        self.send_buffer_size = size;
    }

    /// Sets the value of the SO_RCVBUF option on the socket.
    #[inline]
    pub fn set_recv_buffer_size(&mut self, size: Option<usize>) {
        self.recv_buffer_size = size;
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

    /// Set timeout for hostname resolution.
    ///
    /// If `None`, then no timeout is applied by the connector, making it
    /// subject to the timeout imposed by the operating system.
    ///
    /// Default is `None`.
    #[inline]
    pub fn set_resolve_timeout(&mut self, dur: Option<Duration>) {
        self.resolve_timeout = dur;
    }

    /// Set the connect timeout.
    ///
    /// If a domain resolves to multiple IP addresses, the timeout will be
    /// evenly divided across them.
    ///
    /// Default is `None`.
    #[inline]
    pub fn set_connect_timeout(&mut self, dur: Option<Duration>) {
        self.connect_timeout = dur;
    }

    /// Set timeout for [RFC 6555 (Happy Eyeballs)][RFC 6555] algorithm.
    ///
    /// If hostname resolves to both IPv4 and IPv6 addresses and connection
    /// cannot be established using preferred address family before timeout
    /// elapses, then connector will in parallel attempt connection using other
    /// address family.
    ///
    /// If `None`, parallel connection attempts are disabled.
    ///
    /// Default is 300 milliseconds.
    ///
    /// [RFC 6555]: https://tools.ietf.org/html/rfc6555
    #[inline]
    pub fn set_happy_eyeballs_timeout(&mut self, dur: Option<Duration>) {
        self.happy_eyeballs_timeout = dur;
    }

    /// Set that all socket have `SO_REUSEADDR` set to the supplied value `reuse_address`.
    ///
    /// Default is `false`.
    #[inline]
    pub fn set_reuse_address(&mut self, reuse_address: bool) -> &mut Self {
        self.reuse_address = reuse_address;
        self
    }
}

// R: Debug required for now to allow adding it to debug output later...
impl<R: fmt::Debug> fmt::Debug for HttpConnector<R> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("HttpConnector")
            .finish()
    }
}

impl<R> Connect for HttpConnector<R>
where
    R: Resolve + Clone + Send + Sync,
    R::Future: Send,
{
    type Transport = TcpStream;
    type Error = io::Error;
    type Future = HttpConnecting<R>;

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
        let port = match dst.uri.port_part() {
            Some(port) => port.as_u16(),
            None => if dst.uri.scheme_part() == Some(&Scheme::HTTPS) { 443 } else { 80 },
        };

        HttpConnecting {
            state: State::Lazy(self.resolver.clone(), host.into(), self.local_address),
            handle: self.handle.clone(),
            resolve_timeout: self.resolve_timeout,
            connect_timeout: self.connect_timeout,
            happy_eyeballs_timeout: self.happy_eyeballs_timeout,
            keep_alive_timeout: self.keep_alive_timeout,
            nodelay: self.nodelay,
            port,
            reuse_address: self.reuse_address,
            send_buffer_size: self.send_buffer_size,
            recv_buffer_size: self.recv_buffer_size,
        }
    }
}

impl HttpInfo {
    /// Get the remote address of the transport used.
    pub fn remote_addr(&self) -> SocketAddr {
        self.remote_addr
    }
}

#[inline]
fn invalid_url<R: Resolve>(err: InvalidUrl, handle: &Option<Handle>) -> HttpConnecting<R> {
    HttpConnecting {
        state: State::Error(Some(io::Error::new(io::ErrorKind::InvalidInput, err))),
        handle: handle.clone(),
        keep_alive_timeout: None,
        nodelay: false,
        port: 0,
        resolve_timeout: None,
        connect_timeout: None,
        happy_eyeballs_timeout: None,
        reuse_address: false,
        send_buffer_size: None,
        recv_buffer_size: None,
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
pub struct HttpConnecting<R: Resolve = GaiResolver> {
    state: State<R>,
    handle: Option<Handle>,
    resolve_timeout: Option<Duration>,
    connect_timeout: Option<Duration>,
    happy_eyeballs_timeout: Option<Duration>,
    keep_alive_timeout: Option<Duration>,
    nodelay: bool,
    port: u16,
    reuse_address: bool,
    send_buffer_size: Option<usize>,
    recv_buffer_size: Option<usize>,
}

enum State<R: Resolve> {
    Lazy(R, String, Option<IpAddr>),
    Resolving(ResolvingFuture<R>, Option<IpAddr>),
    Connecting(ConnectingTcp),
    Error(Option<io::Error>),
}

enum ResolvingFuture<R: Resolve> {
    Timed(Timeout<R::Future>),
    Untimed(R::Future),
}

impl<R: Resolve> Future for HttpConnecting<R> {
    type Item = (TcpStream, Connected);
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        loop {
            let state;
            match self.state {
                State::Lazy(ref resolver, ref mut host, local_addr) => {
                    // If the host is already an IP addr (v4 or v6),
                    // skip resolving the dns and start connecting right away.
                    if let Some(addrs) = dns::IpAddrs::try_parse(host, self.port) {
                        state = State::Connecting(ConnectingTcp::new(
                            local_addr, addrs, self.connect_timeout, self.happy_eyeballs_timeout, self.reuse_address));
                    } else {
                        let name = dns::Name::new(mem::replace(host, String::new()));
                        let future = resolver.resolve(name);
                        state = if let Some(timeout) = self.resolve_timeout {
                            State::Resolving(ResolvingFuture::Timed(Timeout::new(future, timeout)), local_addr)
                        } else {
                            State::Resolving(ResolvingFuture::Untimed(future), local_addr)
                        }
                    }
                },
                State::Resolving(ref mut rfuture, local_addr) => {
                    let res: Async<R::Addrs> = match rfuture {
                        ResolvingFuture::Timed(future) => match future.poll() {
                            Ok(res) => res,
                            Err(err) => if err.is_inner() {
                                return Err(err.into_inner().unwrap())
                            } else {
                                return Err(io::Error::new(io::ErrorKind::TimedOut, err.description()))
                            },
                        },
                        ResolvingFuture::Untimed(future) => future.poll()?,
                    };
                    match res {
                        Async::NotReady => return Ok(Async::NotReady),
                        Async::Ready(addrs) => {
                            let port = self.port;
                            let addrs = addrs
                                .map(|addr| SocketAddr::new(addr, port))
                                .collect();
                            let addrs = dns::IpAddrs::new(addrs);
                            state = State::Connecting(ConnectingTcp::new(
                                local_addr, addrs, self.connect_timeout, self.happy_eyeballs_timeout, self.reuse_address));
                        }
                    };
                },
                State::Connecting(ref mut c) => {
                    let sock = try_ready!(c.poll(&self.handle));

                    if let Some(dur) = self.keep_alive_timeout {
                        sock.set_keepalive(Some(dur))?;
                    }

                    if let Some(size) = self.send_buffer_size {
                        sock.set_send_buffer_size(size)?;
                    }

                    if let Some(size) = self.recv_buffer_size {
                        sock.set_recv_buffer_size(size)?;
                    }

                    sock.set_nodelay(self.nodelay)?;

                    let extra = HttpInfo {
                        remote_addr: sock.peer_addr()?,
                    };
                    let connected = Connected::new()
                        .extra(extra);

                    return Ok(Async::Ready((sock, connected)));
                },
                State::Error(ref mut e) => return Err(e.take().expect("polled more than once")),
            }
            self.state = state;
        }
    }
}

impl<R: Resolve + fmt::Debug> fmt::Debug for HttpConnecting<R> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad("HttpConnecting")
    }
}

struct ConnectingTcp {
    local_addr: Option<IpAddr>,
    preferred: ConnectingTcpRemote,
    fallback: Option<ConnectingTcpFallback>,
    reuse_address: bool,
}

impl ConnectingTcp {
    fn new(
        local_addr: Option<IpAddr>,
        remote_addrs: dns::IpAddrs,
        connect_timeout: Option<Duration>,
        fallback_timeout: Option<Duration>,
        reuse_address: bool,
    ) -> ConnectingTcp {
        if let Some(fallback_timeout) = fallback_timeout {
            let (preferred_addrs, fallback_addrs) = remote_addrs.split_by_preference();
            if fallback_addrs.is_empty() {
                return ConnectingTcp {
                    local_addr,
                    preferred: ConnectingTcpRemote::new(preferred_addrs, connect_timeout),
                    fallback: None,
                    reuse_address,
                };
            }

            ConnectingTcp {
                local_addr,
                preferred: ConnectingTcpRemote::new(preferred_addrs, connect_timeout),
                fallback: Some(ConnectingTcpFallback {
                    delay: Delay::new(Instant::now() + fallback_timeout),
                    remote: ConnectingTcpRemote::new(fallback_addrs, connect_timeout),
                }),
                reuse_address,
            }
        } else {
            ConnectingTcp {
                local_addr,
                preferred: ConnectingTcpRemote::new(remote_addrs, connect_timeout),
                fallback: None,
                reuse_address,
            }
        }
    }
}

struct ConnectingTcpFallback {
    delay: Delay,
    remote: ConnectingTcpRemote,
}

struct ConnectingTcpRemote {
    addrs: dns::IpAddrs,
    connect_timeout: Option<Duration>,
    current: Option<MaybeTimedConnectFuture>,
}

impl ConnectingTcpRemote {
    fn new(addrs: dns::IpAddrs, connect_timeout: Option<Duration>) -> Self {
        let connect_timeout = connect_timeout.map(|t| t / (addrs.len() as u32));

        Self {
            addrs,
            connect_timeout,
            current: None,
        }
    }
}

impl ConnectingTcpRemote {
    // not a Future, since passing a &Handle to poll
    fn poll(
        &mut self,
        local_addr: &Option<IpAddr>,
        handle: &Option<Handle>,
        reuse_address: bool,
    ) -> Poll<TcpStream, io::Error> {
        let mut err = None;
        loop {
            if let Some(ref mut current) = self.current {
                let poll: Poll<TcpStream, io::Error> = match current {
                    MaybeTimedConnectFuture::Timed(future) => match future.poll() {
                        Ok(tcp) => Ok(tcp),
                        Err(err) => if err.is_inner() {
                            Err(err.into_inner().unwrap())
                        } else {
                            Err(io::Error::new(io::ErrorKind::TimedOut, err.description()))
                        }
                    },
                    MaybeTimedConnectFuture::Untimed(future) => future.poll(),
                };
                match poll {
                    Ok(Async::Ready(tcp)) => {
                        debug!("connected to {:?}", tcp.peer_addr().ok());
                        return Ok(Async::Ready(tcp));
                    },
                    Ok(Async::NotReady) => return Ok(Async::NotReady),
                    Err(e) => {
                        trace!("connect error {:?}", e);
                        err = Some(e);
                        if let Some(addr) = self.addrs.next() {
                            debug!("connecting to {}", addr);
                            *current = connect(&addr, local_addr, handle, reuse_address, self.connect_timeout)?;
                            continue;
                        }
                    }
                }
            } else if let Some(addr) = self.addrs.next() {
                debug!("connecting to {}", addr);
                self.current = Some(connect(&addr, local_addr, handle, reuse_address, self.connect_timeout)?);
                continue;
            }

            return Err(err.take().expect("missing connect error"));
        }
    }
}

enum MaybeTimedConnectFuture {
    Timed(Timeout<ConnectFuture>),
    Untimed(ConnectFuture),
}

fn connect(addr: &SocketAddr, local_addr: &Option<IpAddr>, handle: &Option<Handle>, reuse_address: bool, connect_timeout: Option<Duration>) -> io::Result<MaybeTimedConnectFuture> {
    let builder = match addr {
        &SocketAddr::V4(_) => TcpBuilder::new_v4()?,
        &SocketAddr::V6(_) => TcpBuilder::new_v6()?,
    };

    if reuse_address {
        builder.reuse_address(reuse_address)?;
    }

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
        None => Cow::Owned(Handle::default()),
    };

    let stream = TcpStream::connect_std(builder.to_tcp_stream()?, addr, &handle);

    if let Some(timeout) = connect_timeout {
        Ok(MaybeTimedConnectFuture::Timed(Timeout::new(stream, timeout)))
    } else {
        Ok(MaybeTimedConnectFuture::Untimed(stream))
    }
}

impl ConnectingTcp {
    // not a Future, since passing a &Handle to poll
    fn poll(&mut self, handle: &Option<Handle>) -> Poll<TcpStream, io::Error> {
        match self.fallback.take() {
            None => self.preferred.poll(&self.local_addr, handle, self.reuse_address),
            Some(mut fallback) => match self.preferred.poll(&self.local_addr, handle, self.reuse_address) {
                Ok(Async::Ready(stream)) => {
                    // Preferred successful - drop fallback.
                    Ok(Async::Ready(stream))
                }
                Ok(Async::NotReady) => match fallback.delay.poll() {
                    Ok(Async::Ready(_)) => match fallback.remote.poll(&self.local_addr, handle, self.reuse_address) {
                        Ok(Async::Ready(stream)) => {
                            // Fallback successful - drop current preferred,
                            // but keep fallback as new preferred.
                            self.preferred = fallback.remote;
                            Ok(Async::Ready(stream))
                        }
                        Ok(Async::NotReady) => {
                            // Neither preferred nor fallback are ready.
                            self.fallback = Some(fallback);
                            Ok(Async::NotReady)
                        }
                        Err(_) => {
                            // Fallback failed - resume with preferred only.
                            Ok(Async::NotReady)
                        }
                    },
                    Ok(Async::NotReady) => {
                        // Too early to attempt fallback.
                        self.fallback = Some(fallback);
                        Ok(Async::NotReady)
                    }
                    Err(_) => {
                        // Fallback delay failed - resume with preferred only.
                        Ok(Async::NotReady)
                    }
                }
                Err(_) => {
                    // Preferred failed - use fallback as new preferred.
                    self.preferred = fallback.remote;
                    self.preferred.poll(&self.local_addr, handle, self.reuse_address)
                }
            }
        }
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

    #[test]
    #[cfg_attr(not(feature = "__internal_happy_eyeballs_tests"), ignore)]
    fn client_happy_eyeballs() {
        extern crate pretty_env_logger;

        use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, TcpListener};
        use std::time::{Duration, Instant};

        use futures::{Async, Poll};
        use tokio::runtime::current_thread::Runtime;
        use tokio_reactor::Handle;

        use super::dns;
        use super::ConnectingTcp;

        let _ = pretty_env_logger::try_init();
        let server4 = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = server4.local_addr().unwrap();
        let _server6 = TcpListener::bind(&format!("[::1]:{}", addr.port())).unwrap();
        let mut rt = Runtime::new().unwrap();

        let local_timeout = Duration::default();
        let unreachable_v4_timeout = measure_connect(unreachable_ipv4_addr()).1;
        let unreachable_v6_timeout = measure_connect(unreachable_ipv6_addr()).1;
        let fallback_timeout = ::std::cmp::max(unreachable_v4_timeout, unreachable_v6_timeout)
            + Duration::from_millis(250);

        let scenarios = &[
            // Fast primary, without fallback.
            (&[local_ipv4_addr()][..],
                4, local_timeout, false),
            (&[local_ipv6_addr()][..],
                6, local_timeout, false),

            // Fast primary, with (unused) fallback.
            (&[local_ipv4_addr(), local_ipv6_addr()][..],
                4, local_timeout, false),
            (&[local_ipv6_addr(), local_ipv4_addr()][..],
                6, local_timeout, false),

            // Unreachable + fast primary, without fallback.
            (&[unreachable_ipv4_addr(), local_ipv4_addr()][..],
                4, unreachable_v4_timeout, false),
            (&[unreachable_ipv6_addr(), local_ipv6_addr()][..],
                6, unreachable_v6_timeout, false),

            // Unreachable + fast primary, with (unused) fallback.
            (&[unreachable_ipv4_addr(), local_ipv4_addr(), local_ipv6_addr()][..],
                4, unreachable_v4_timeout, false),
            (&[unreachable_ipv6_addr(), local_ipv6_addr(), local_ipv4_addr()][..],
                6, unreachable_v6_timeout, true),

            // Slow primary, with (used) fallback.
            (&[slow_ipv4_addr(), local_ipv4_addr(), local_ipv6_addr()][..],
                6, fallback_timeout, false),
            (&[slow_ipv6_addr(), local_ipv6_addr(), local_ipv4_addr()][..],
                4, fallback_timeout, true),

            // Slow primary, with (used) unreachable + fast fallback.
            (&[slow_ipv4_addr(), unreachable_ipv6_addr(), local_ipv6_addr()][..],
                6, fallback_timeout + unreachable_v6_timeout, false),
            (&[slow_ipv6_addr(), unreachable_ipv4_addr(), local_ipv4_addr()][..],
                4, fallback_timeout + unreachable_v4_timeout, true),
        ];

        // Scenarios for IPv6 -> IPv4 fallback require that host can access IPv6 network.
        // Otherwise, connection to "slow" IPv6 address will error-out immediatelly.
        let ipv6_accessible = measure_connect(slow_ipv6_addr()).0;

        for &(hosts, family, timeout, needs_ipv6_access) in scenarios {
            if needs_ipv6_access && !ipv6_accessible {
                continue;
            }

            let addrs = hosts.iter().map(|host| (host.clone(), addr.port()).into()).collect();
            let connecting_tcp = ConnectingTcp::new(None, dns::IpAddrs::new(addrs), None, Some(fallback_timeout), false);
            let fut = ConnectingTcpFuture(connecting_tcp);

            let start = Instant::now();
            let res = rt.block_on(fut).unwrap();
            let duration = start.elapsed();

            // Allow actual duration to be +/- 150ms off.
            let min_duration = if timeout >= Duration::from_millis(150) {
                timeout - Duration::from_millis(150)
            } else {
                Duration::default()
            };
            let max_duration = timeout + Duration::from_millis(150);

            assert_eq!(res, family);
            assert!(duration >= min_duration);
            assert!(duration <= max_duration);
        }

        struct ConnectingTcpFuture(ConnectingTcp);

        impl Future for ConnectingTcpFuture {
            type Item = u8;
            type Error = ::std::io::Error;

            fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
                match self.0.poll(&Some(Handle::default())) {
                    Ok(Async::Ready(stream)) => Ok(Async::Ready(
                        if stream.peer_addr().unwrap().is_ipv4() { 4 } else { 6 }
                    )),
                    Ok(Async::NotReady) => Ok(Async::NotReady),
                    Err(err) => Err(err),
                }
            }
        }

        fn local_ipv4_addr() -> IpAddr {
            Ipv4Addr::new(127, 0, 0, 1).into()
        }

        fn local_ipv6_addr() -> IpAddr {
            Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1).into()
        }

        fn unreachable_ipv4_addr() -> IpAddr {
            Ipv4Addr::new(127, 0, 0, 2).into()
        }

        fn unreachable_ipv6_addr() -> IpAddr {
            Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 2).into()
        }

        fn slow_ipv4_addr() -> IpAddr {
            // RFC 6890 reserved IPv4 address.
            Ipv4Addr::new(198, 18, 0, 25).into()
        }

        fn slow_ipv6_addr() -> IpAddr {
            // RFC 6890 reserved IPv6 address.
            Ipv6Addr::new(2001, 2, 0, 0, 0, 0, 0, 254).into()
        }

        fn measure_connect(addr: IpAddr) -> (bool, Duration) {
            let start = Instant::now();
            let result = ::std::net::TcpStream::connect_timeout(
                &(addr, 80).into(), Duration::from_secs(1));

            let reachable = result.is_ok() || result.unwrap_err().kind() == io::ErrorKind::TimedOut;
            let duration = start.elapsed();
            (reachable, duration)
        }
    }
}

