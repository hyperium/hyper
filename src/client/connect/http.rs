use std::fmt;
use std::error::Error as StdError;
use std::io;
use std::mem;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use http::uri::{Scheme, Uri};
use futures_util::{TryFutureExt};
use net2::TcpBuilder;
use pin_project::{pin_project, project};
use tokio::net::TcpStream;
use tokio::time::Delay;

use crate::common::{Future, Pin, Poll, task};
use super::{Connected, Destination};
use super::dns::{self, GaiResolver, Resolve};
//#[cfg(feature = "runtime")] use super::dns::TokioThreadpoolGaiResolver;

// TODO: unbox me?
type ConnectFuture = Pin<Box<dyn Future<Output = io::Result<TcpStream>> + Send>>;

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
    config: Arc<Config>,
    resolver: R,
}

/// Extra information about the transport when an HttpConnector is used.
///
/// # Example
///
/// ```
/// # async fn doc() -> hyper::Result<()> {
/// use hyper::Uri;
/// use hyper::client::{Client, connect::HttpInfo};
///
/// let client = Client::new();
/// let uri = Uri::from_static("http://example.com");
///
/// let res = client.get(uri).await?;
/// res
///     .extensions()
///     .get::<HttpInfo>()
///     .map(|info| {
///         println!("remote addr = {}", info.remote_addr());
///     });
/// # Ok(())
/// # }
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

#[derive(Clone)]
struct Config {
    connect_timeout: Option<Duration>,
    enforce_http: bool,
    happy_eyeballs_timeout: Option<Duration>,
    keep_alive_timeout: Option<Duration>,
    local_address: Option<IpAddr>,
    nodelay: bool,
    reuse_address: bool,
    send_buffer_size: Option<usize>,
    recv_buffer_size: Option<usize>,
}

// ===== impl HttpConnector =====

impl HttpConnector {
    /// Construct a new HttpConnector.
    pub fn new() -> HttpConnector {
        HttpConnector::new_with_resolver(GaiResolver::new())
    }
}

/*
#[cfg(feature = "runtime")]
impl HttpConnector<TokioThreadpoolGaiResolver> {
    /// Construct a new HttpConnector using the `TokioThreadpoolGaiResolver`.
    ///
    /// This resolver **requires** the threadpool runtime to be used.
    pub fn new_with_tokio_threadpool_resolver() -> Self {
        HttpConnector::new_with_resolver(TokioThreadpoolGaiResolver::new())
    }
}
*/


impl<R> HttpConnector<R> {
    /// Construct a new HttpConnector.
    ///
    /// Takes a `Resolve` to handle DNS lookups.
    pub fn new_with_resolver(resolver: R) -> HttpConnector<R> {
        HttpConnector {
            config: Arc::new(Config {
                connect_timeout: None,
                enforce_http: true,
                happy_eyeballs_timeout: Some(Duration::from_millis(300)),
                keep_alive_timeout: None,
                local_address: None,
                nodelay: false,
                reuse_address: false,
                send_buffer_size: None,
                recv_buffer_size: None,
            }),
            resolver,
        }
    }

    /// Option to enforce all `Uri`s have the `http` scheme.
    ///
    /// Enabled by default.
    #[inline]
    pub fn enforce_http(&mut self, is_enforced: bool) {
        self.config_mut().enforce_http = is_enforced;
    }

    /// Set that all sockets have `SO_KEEPALIVE` set with the supplied duration.
    ///
    /// If `None`, the option will not be set.
    ///
    /// Default is `None`.
    #[inline]
    pub fn set_keepalive(&mut self, dur: Option<Duration>) {
        self.config_mut().keep_alive_timeout = dur;
    }

    /// Set that all sockets have `SO_NODELAY` set to the supplied value `nodelay`.
    ///
    /// Default is `false`.
    #[inline]
    pub fn set_nodelay(&mut self, nodelay: bool) {
        self.config_mut().nodelay = nodelay;
    }

    /// Sets the value of the SO_SNDBUF option on the socket.
    #[inline]
    pub fn set_send_buffer_size(&mut self, size: Option<usize>) {
        self.config_mut().send_buffer_size = size;
    }

    /// Sets the value of the SO_RCVBUF option on the socket.
    #[inline]
    pub fn set_recv_buffer_size(&mut self, size: Option<usize>) {
        self.config_mut().recv_buffer_size = size;
    }

    /// Set that all sockets are bound to the configured address before connection.
    ///
    /// If `None`, the sockets will not be bound.
    ///
    /// Default is `None`.
    #[inline]
    pub fn set_local_address(&mut self, addr: Option<IpAddr>) {
        self.config_mut().local_address = addr;
    }

    /// Set the connect timeout.
    ///
    /// If a domain resolves to multiple IP addresses, the timeout will be
    /// evenly divided across them.
    ///
    /// Default is `None`.
    #[inline]
    pub fn set_connect_timeout(&mut self, dur: Option<Duration>) {
        self.config_mut().connect_timeout = dur;
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
        self.config_mut().happy_eyeballs_timeout = dur;
    }

    /// Set that all socket have `SO_REUSEADDR` set to the supplied value `reuse_address`.
    ///
    /// Default is `false`.
    #[inline]
    pub fn set_reuse_address(&mut self, reuse_address: bool) -> &mut Self {
        self.config_mut().reuse_address = reuse_address;
        self
    }

    // private

    fn config_mut(&mut self) -> &mut Config {
        // If the are HttpConnector clones, this will clone the inner
        // config. So mutating the config won't ever affect previous
        // clones.
        Arc::make_mut(&mut self.config)
    }
}

static INVALID_NOT_HTTP: &str = "invalid URL, scheme is not http";
static INVALID_MISSING_SCHEME: &str = "invalid URL, scheme is missing";
static INVALID_MISSING_HOST: &str = "invalid URL, host is missing";

impl<R: Resolve> HttpConnector<R> {
    fn invalid_url(&self, msg: impl Into<Box<str>>) -> HttpConnecting<R> {
        HttpConnecting {
            config: self.config.clone(),
            state: State::Error(Some(ConnectError {
                msg: msg.into(),
                cause: None,
            })),
            port: 0,
        }
    }
}

// R: Debug required for now to allow adding it to debug output later...
impl<R: fmt::Debug> fmt::Debug for HttpConnector<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HttpConnector")
            .finish()
    }
}

impl<R> tower_service::Service<Destination> for HttpConnector<R>
where
    R: Resolve + Clone + Send + Sync,
    R::Future: Send,
{
    type Response = (TcpStream, Connected);
    type Error = ConnectError;
    type Future = HttpConnecting<R>;

    fn poll_ready(&mut self, cx: &mut task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        ready!(self.resolver.poll_ready(cx)).map_err(ConnectError::dns)?;
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, dst: Destination) -> Self::Future {
        trace!(
            "Http::connect; scheme={}, host={}, port={:?}",
            dst.scheme(),
            dst.host(),
            dst.port(),
        );

        if self.config.enforce_http {
            if dst.uri.scheme() != Some(&Scheme::HTTP) {
                return self.invalid_url(INVALID_NOT_HTTP);
            }
        } else if dst.uri.scheme().is_none() {
            return self.invalid_url(INVALID_MISSING_SCHEME);
        }

        let host = match dst.uri.host() {
            Some(s) => s,
            None => return self.invalid_url(INVALID_MISSING_HOST),
        };
        let port = match dst.uri.port() {
            Some(port) => port.as_u16(),
            None => if dst.uri.scheme() == Some(&Scheme::HTTPS) { 443 } else { 80 },
        };

        HttpConnecting {
            config: self.config.clone(),
            state: State::Lazy(self.resolver.clone(), host.into()),
            port,
        }
    }
}

impl<R> tower_service::Service<Uri> for HttpConnector<R>
where
    R: Resolve + Clone + Send + Sync + 'static,
    R::Future: Send,
{
    type Response = TcpStream;
    type Error = ConnectError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + 'static>>;

    fn poll_ready(&mut self, cx: &mut task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        tower_service::Service::<Destination>::poll_ready(self, cx)
    }

    fn call(&mut self, uri: Uri) -> Self::Future {
        Box::pin(self.call(Destination { uri }).map_ok(|(s, _)| s))
    }
}

impl HttpInfo {
    /// Get the remote address of the transport used.
    pub fn remote_addr(&self) -> SocketAddr {
        self.remote_addr
    }
}

// Not publicly exported (so missing_docs doesn't trigger).
pub struct ConnectError {
    msg: Box<str>,
    cause: Option<Box<dyn StdError + Send + Sync>>,
}

impl ConnectError {
    fn new<S, E>(msg: S, cause: E) -> ConnectError
    where
        S: Into<Box<str>>,
        E: Into<Box<dyn StdError + Send + Sync>>,
    {
        ConnectError {
            msg: msg.into(),
            cause: Some(cause.into()),
        }
    }

    fn dns<E>(cause: E) -> ConnectError
    where
        E: Into<Box<dyn StdError + Send + Sync>>,
    {
        ConnectError::new("dns error", cause)
    }

    fn m<S, E>(msg: S) -> impl FnOnce(E) -> ConnectError
    where
        S: Into<Box<str>>,
        E: Into<Box<dyn StdError + Send + Sync>>,
    {
        move |cause| {
            ConnectError::new(msg, cause)
        }
    }
}

impl fmt::Debug for ConnectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(ref cause) = self.cause {
            f.debug_tuple("ConnectError")
                .field(&self.msg)
                .field(cause)
                .finish()
        } else {
            self.msg.fmt(f)
        }
    }
}

impl fmt::Display for ConnectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.msg)?;

        if let Some(ref cause) = self.cause {
            write!(f, ": {}", cause)?;
        }

        Ok(())
    }
}

impl StdError for ConnectError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.cause.as_ref().map(|e| &**e as _)
    }
}

/// A Future representing work to connect to a URL.
#[must_use = "futures do nothing unless polled"]
#[pin_project]
pub struct HttpConnecting<R: Resolve = GaiResolver> {
    config: Arc<Config>,
    #[pin]
    state: State<R>,
    port: u16,
}

#[pin_project]
enum State<R: Resolve> {
    Lazy(R, String),
    Resolving(#[pin] R::Future),
    Connecting(ConnectingTcp),
    Error(Option<ConnectError>),
}

impl<R: Resolve> Future for HttpConnecting<R> {
    type Output = Result<(TcpStream, Connected), ConnectError>;

    #[project]
    fn poll(self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Self::Output> {
        let mut me = self.project();
        let config: &Config = &me.config;
        loop {
            let state;
            #[project]
            match me.state.as_mut().project() {
                State::Lazy(ref mut resolver, ref mut host) => {
                    // If the host is already an IP addr (v4 or v6),
                    // skip resolving the dns and start connecting right away.
                    if let Some(addrs) = dns::IpAddrs::try_parse(host, *me.port) {
                        state = State::Connecting(ConnectingTcp::new(
                            config.local_address, addrs, config.connect_timeout, config.happy_eyeballs_timeout, config.reuse_address));
                    } else {
                        ready!(resolver.poll_ready(cx)).map_err(ConnectError::dns)?;
                        let name = dns::Name::new(mem::replace(host, String::new()));
                        state = State::Resolving(resolver.resolve(name));
                    }
                },
                State::Resolving(future) => {
                    let addrs =  ready!(future.poll(cx)).map_err(ConnectError::dns)?;
                    let port = *me.port;
                    let addrs = addrs
                        .map(|addr| SocketAddr::new(addr, port))
                        .collect();
                    let addrs = dns::IpAddrs::new(addrs);
                    state = State::Connecting(ConnectingTcp::new(
                        config.local_address, addrs, config.connect_timeout, config.happy_eyeballs_timeout, config.reuse_address));
                },
                State::Connecting(ref mut c) => {
                    let sock = ready!(c.poll(cx))
                        .map_err(ConnectError::m("tcp connect error"))?;

                    if let Some(dur) = config.keep_alive_timeout {
                        sock.set_keepalive(Some(dur)).map_err(ConnectError::m("tcp set_keepalive error"))?;
                    }

                    if let Some(size) = config.send_buffer_size {
                        sock.set_send_buffer_size(size).map_err(ConnectError::m("tcp set_send_buffer_size error"))?;
                    }

                    if let Some(size) = config.recv_buffer_size {
                        sock.set_recv_buffer_size(size).map_err(ConnectError::m("tcp set_recv_buffer_size error"))?;
                    }

                    sock.set_nodelay(config.nodelay).map_err(ConnectError::m("tcp set_nodelay error"))?;

                    let extra = HttpInfo {
                        remote_addr: sock.peer_addr().map_err(ConnectError::m("tcp peer_addr error"))?,
                    };
                    let connected = Connected::new()
                        .extra(extra);

                    return Poll::Ready(Ok((sock, connected)));
                },
                State::Error(ref mut e) => return Poll::Ready(Err(e.take().expect("polled more than once"))),
            }
            me.state.set(state);
        }
    }
}

impl<R: Resolve + fmt::Debug> fmt::Debug for HttpConnecting<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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
            let (preferred_addrs, fallback_addrs) = remote_addrs.split_by_preference(local_addr);
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
                    delay: tokio::time::delay_for(fallback_timeout),
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
    current: Option<ConnectFuture>,
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
    fn poll(
        &mut self,
        cx: &mut task::Context<'_>,
        local_addr: &Option<IpAddr>,
        reuse_address: bool,
    ) -> Poll<io::Result<TcpStream>> {
        let mut err = None;
        loop {
            if let Some(ref mut current) = self.current {
                match current.as_mut().poll(cx) {
                    Poll::Ready(Ok(tcp)) => {
                        debug!("connected to {:?}", tcp.peer_addr().ok());
                        return Poll::Ready(Ok(tcp));
                    },
                    Poll::Pending => return Poll::Pending,
                    Poll::Ready(Err(e)) => {
                        trace!("connect error {:?}", e);
                        err = Some(e);
                        if let Some(addr) = self.addrs.next() {
                            debug!("connecting to {}", addr);
                            *current = connect(&addr, local_addr, reuse_address, self.connect_timeout)?;
                            continue;
                        }
                    }
                }
            } else if let Some(addr) = self.addrs.next() {
                debug!("connecting to {}", addr);
                self.current = Some(connect(&addr, local_addr, reuse_address, self.connect_timeout)?);
                continue;
            }

            return Poll::Ready(Err(err.take().expect("missing connect error")));
        }
    }
}

fn connect(
    addr: &SocketAddr,
    local_addr: &Option<IpAddr>,
    reuse_address: bool,
    connect_timeout: Option<Duration>,
) -> io::Result<ConnectFuture> {
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
    } else if cfg!(windows) {
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

    let addr = *addr;

    let std_tcp = builder.to_tcp_stream()?;

    Ok(Box::pin(async move {
        let connect = TcpStream::connect_std(std_tcp, &addr);
        match connect_timeout {
            Some(dur) => match tokio::time::timeout(dur, connect).await {
                Ok(Ok(s)) => Ok(s),
                Ok(Err(e)) => Err(e),
                Err(e) => Err(io::Error::new(io::ErrorKind::TimedOut, e)),
            }
            None => connect.await,
        }
    }))
}

impl ConnectingTcp {
    fn poll(&mut self, cx: &mut task::Context<'_>) -> Poll<io::Result<TcpStream>> {
        match self.fallback.take() {
            None => self.preferred.poll(cx, &self.local_addr, self.reuse_address),
            Some(mut fallback) => match self.preferred.poll(cx, &self.local_addr, self.reuse_address) {
                Poll::Ready(Ok(stream)) => {
                    // Preferred successful - drop fallback.
                    Poll::Ready(Ok(stream))
                }
                Poll::Pending => match Pin::new(&mut fallback.delay).poll(cx) {
                    Poll::Ready(()) => match fallback.remote.poll(cx, &self.local_addr, self.reuse_address) {
                        Poll::Ready(Ok(stream)) => {
                            // Fallback successful - drop current preferred,
                            // but keep fallback as new preferred.
                            self.preferred = fallback.remote;
                            Poll::Ready(Ok(stream))
                        }
                        Poll::Pending => {
                            // Neither preferred nor fallback are ready.
                            self.fallback = Some(fallback);
                            Poll::Pending
                        }
                        Poll::Ready(Err(_)) => {
                            // Fallback failed - resume with preferred only.
                            Poll::Pending
                        }
                    },
                    Poll::Pending => {
                        // Too early to attempt fallback.
                        self.fallback = Some(fallback);
                        Poll::Pending
                    }
                }
                Poll::Ready(Err(_)) => {
                    // Preferred failed - use fallback as new preferred.
                    self.preferred = fallback.remote;
                    self.preferred.poll(cx, &self.local_addr, self.reuse_address)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io;

    use super::{Connected, Destination, HttpConnector};
    use super::super::sealed::Connect;

    async fn connect<C>(connector: C, dst: Destination) -> Result<(C::Transport, Connected), C::Error>
    where
        C: Connect,
    {
        connector.connect(super::super::sealed::Internal, dst).await
    }

    #[tokio::test]
    async fn test_errors_enforce_http() {
        let uri = "https://example.domain/foo/bar?baz".parse().unwrap();
        let dst = Destination {
            uri,
        };
        let connector = HttpConnector::new();

        let err = connect(connector, dst).await.unwrap_err();
        assert_eq!(&*err.msg, super::INVALID_NOT_HTTP);
    }

    #[tokio::test]
    async fn test_errors_missing_scheme() {
        let uri = "example.domain".parse().unwrap();
        let dst = Destination {
            uri,
        };
        let mut connector = HttpConnector::new();
        connector.enforce_http(false);

        let err = connect(connector, dst).await.unwrap_err();
        assert_eq!(&*err.msg, super::INVALID_MISSING_SCHEME);
    }

    #[test]
    #[cfg_attr(not(feature = "__internal_happy_eyeballs_tests"), ignore)]
    fn client_happy_eyeballs() {
        use std::future::Future;
        use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, TcpListener};
        use std::task::Poll;
        use std::time::{Duration, Instant};

        use crate::common::{Pin, task};
        use super::dns;
        use super::ConnectingTcp;

        let _ = pretty_env_logger::try_init();
        let server4 = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = server4.local_addr().unwrap();
        let _server6 = TcpListener::bind(&format!("[::1]:{}", addr.port())).unwrap();
        let mut rt = tokio::runtime::Builder::new()
            .enable_io()
            .enable_time()
            .basic_scheduler()
            .build()
            .unwrap();

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
            let (res, duration) = rt.block_on(async move {
                let connecting_tcp = ConnectingTcp::new(None, dns::IpAddrs::new(addrs), None, Some(fallback_timeout), false);
                let fut = ConnectingTcpFuture(connecting_tcp);
                let start = Instant::now();
                let res = fut.await.unwrap();
                (res, start.elapsed())
            });

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
            type Output = Result<u8, std::io::Error>;

            fn poll(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Self::Output> {
                match self.0.poll(cx) {
                    Poll::Ready(Ok(stream)) => Poll::Ready(Ok(
                        if stream.peer_addr().unwrap().is_ipv4() { 4 } else { 6 }
                    )),
                    Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
                    Poll::Pending => Poll::Pending,
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

