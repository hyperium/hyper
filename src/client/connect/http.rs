use std::error::Error as StdError;
use std::fmt;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

//#[cfg(feature = "runtime")] use super::dns::TokioThreadpoolGaiResolver;

/// A connector for the `http` scheme.
///
/// Performs DNS resolution in a thread pool, and then connects over TCP.
///
/// # Note
///
/// Sets the [`HttpInfo`](HttpInfo) value on responses, which includes
/// transport information such as the remote socket address used.
#[derive(Clone)]
pub struct HttpConnector {
    config: Arc<Config>,
}

/// Extra information about the transport when an HttpConnector is used.
///
/// # Note
///
/// If a different connector is used besides [`HttpConnector`](HttpConnector),
/// this value will not exist in the extensions. Consult that specific
/// connector to see what "extra" information it might provide to responses.
#[derive(Clone, Debug)]
pub struct HttpInfo {
    remote_addr: SocketAddr,
    local_addr: SocketAddr,
}

#[derive(Clone)]
struct Config {
    connect_timeout: Option<Duration>,
    enforce_http: bool,
    happy_eyeballs_timeout: Option<Duration>,
    keep_alive_timeout: Option<Duration>,
    local_address_ipv4: Option<Ipv4Addr>,
    local_address_ipv6: Option<Ipv6Addr>,
    nodelay: bool,
    reuse_address: bool,
    send_buffer_size: Option<usize>,
    recv_buffer_size: Option<usize>,
}

// ===== impl HttpConnector =====

impl HttpConnector {
    /// Construct a new HttpConnector.
    pub fn new() -> HttpConnector {
        HttpConnector {
            config: Arc::new(Config {
                connect_timeout: None,
                enforce_http: true,
                happy_eyeballs_timeout: Some(Duration::from_millis(300)),
                keep_alive_timeout: None,
                local_address_ipv4: None,
                local_address_ipv6: None,
                nodelay: false,
                reuse_address: false,
                send_buffer_size: None,
                recv_buffer_size: None,
            }),
        }
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

impl HttpConnector {
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
        let (v4, v6) = match addr {
            Some(IpAddr::V4(a)) => (Some(a), None),
            Some(IpAddr::V6(a)) => (None, Some(a)),
            _ => (None, None),
        };

        let cfg = self.config_mut();

        cfg.local_address_ipv4 = v4;
        cfg.local_address_ipv6 = v6;
    }

    /// Set that all sockets are bound to the configured IPv4 or IPv6 address (depending on host's
    /// preferences) before connection.
    #[inline]
    pub fn set_local_addresses(&mut self, addr_ipv4: Ipv4Addr, addr_ipv6: Ipv6Addr) {
        let cfg = self.config_mut();

        cfg.local_address_ipv4 = Some(addr_ipv4);
        cfg.local_address_ipv6 = Some(addr_ipv6);
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

// R: Debug required for now to allow adding it to debug output later...
impl fmt::Debug for HttpConnector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HttpConnector").finish()
    }
}

impl HttpInfo {
    /// Get the remote address of the transport used.
    pub fn remote_addr(&self) -> SocketAddr {
        self.remote_addr
    }

    /// Get the local address of the transport used.
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }
}

// Not publicly exported (so missing_docs doesn't trigger).
pub(crate) struct ConnectError {
    msg: Box<str>,
    cause: Option<Box<dyn StdError + Send + Sync>>,
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
