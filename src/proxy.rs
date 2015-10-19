//! Client Proxy support.
use std::net::TcpStream;
use std::io;

use Client;
use net::{HttpStream, NetworkConnector};
use http::h1::Http11Protocol;
use method::Method;
use std::fmt;
use url::Url;

use header::ProxyConnection;


#[derive(Copy)]
pub enum ProxyPolicy {
    /// Proxy all requests.
    ProxyAll,
    /// Proxy only http requests.
    ProxyHttp,
    /// Proxy only https requestss.
    ProxyHttps,
    /// Proxy if the contained function returns true.
    ProxyIf(fn(&Url) -> bool),
}

// Need to implement Clone for the time being.
impl Clone for ProxyPolicy {
    fn clone(&self) -> ProxyPolicy {
        *self
    }
}

impl Default for ProxyPolicy {
    fn default() -> ProxyPolicy {
        ProxyPolicy::ProxyAll
    }
}

impl fmt::Debug for ProxyPolicy {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ProxyPolicy::ProxyAll => f.write_str("ProxyPolicy::ProxyAll"),
            ProxyPolicy::ProxyHttp => f.write_str("ProxyPolicy::ProxyHttp"),
            ProxyPolicy::ProxyHttps => f.write_str("ProxyPolicy::ProxyHttps"),
            ProxyPolicy::ProxyIf(_) => f.write_str("ProxyPolicy::ProxyIf(_)")
        }
    }
}

impl ProxyPolicy {
    pub fn can_handle(&self, url: &Url) -> bool {
        match *self {
            ProxyPolicy::ProxyAll => true,
            ProxyPolicy::ProxyHttp => url.scheme == "http",
            ProxyPolicy::ProxyHttps => url.scheme == "https",
            ProxyPolicy::ProxyIf(f) => f(url)
        }
    }
}

/// Proxy object.
#[derive(Debug, Clone)]
pub struct Proxy {
    config: Config
}

/// Configuration required to connect to the proxy.
#[derive(Debug, Clone)]
pub struct Config {
    /// The proxy host to connect to.
    pub proxy_host: String,
    /// The proxy port to connect to.
    pub proxy_port: u16,
    /// Http version of the proxy.
    pub proxy_version: String,
    /// The policy to enable the proxy.
    pub proxy_policy: ProxyPolicy,
    /// The authorization header, would be nice to leverage header.Authorization.
    pub proxy_authorization: String,
}


/// Passthrough Connector used to negotiate the proxy connection.
struct PassthroughConector {
    stream: HttpStream
}

impl NetworkConnector for PassthroughConector {
    type Stream = HttpStream;

    fn connect(&self, _: &str, _: u16, _: &str) -> ::Result<HttpStream> {
        Ok(self.stream.clone())
    }
}


impl Proxy {
    /// Creates a `Proxy`.
    #[inline]
    pub fn new(config: Config) -> Proxy {
        Proxy {
            config: config
        }
    }

    pub fn can_handle(&self, url: &Url) -> bool {
        self.config.proxy_policy.can_handle(url)
    }

    /// Return a stream connected through the proxy.
    pub fn connect(&self, host: &str, port: u16, scheme: &str) -> io::Result<TcpStream> {
        let proxy_addr = &( &*self.config.proxy_host, self.config.proxy_port);
        let stream = try!(TcpStream::connect(proxy_addr));
        if scheme == "https" {
            let passthrough = PassthroughConector { stream: HttpStream(stream.try_clone().unwrap())};
            let protocol = Http11Protocol::with_connector(passthrough);
            let client = Client::with_protocol(protocol);
            let url = format!("https://{}:{}", host, port);
            let req = client.request(Method::Connect, &url).header(ProxyConnection::keep_alive());
            match req.send() {
                Ok(res) => {
                    debug!("Proxy response: {}", res.status);
                },
                Err(e) => {
                    debug!("Proxy error {:?}", e);
                }
            }
        }
        Ok(stream)
    }

}
