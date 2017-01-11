//! HTTP Client
//!
//! # Usage
//!
//! The `Client` API is designed for most people to make HTTP requests.
//! It utilizes the lower level `Request` API.
//!
//! ## GET
//!
//! ```no_run
//! # use hyper::Client;
//! let client = Client::new();
//!
//! let res = client.get("http://example.domain").send().unwrap();
//! assert_eq!(res.status, hyper::Ok);
//! ```
//!
//! The returned value is a `Response`, which provides easy access to
//! the `status`, the `headers`, and the response body via the `Read`
//! trait.
//!
//! ## POST
//!
//! ```no_run
//! # use hyper::Client;
//! let client = Client::new();
//!
//! let res = client.post("http://example.domain")
//!     .body("foo=bar")
//!     .send()
//!     .unwrap();
//! assert_eq!(res.status, hyper::Ok);
//! ```
//!
//! # Sync
//!
//! The `Client` implements `Sync`, so you can share it among multiple threads
//! and make multiple requests simultaneously.
//!
//! ```no_run
//! # use hyper::Client;
//! use std::sync::Arc;
//! use std::thread;
//!
//! // Note: an Arc is used here because `thread::spawn` creates threads that
//! // can outlive the main thread, so we must use reference counting to keep
//! // the Client alive long enough. Scoped threads could skip the Arc.
//! let client = Arc::new(Client::new());
//! let clone1 = client.clone();
//! let clone2 = client.clone();
//! thread::spawn(move || {
//!     clone1.get("http://example.domain").send().unwrap();
//! });
//! thread::spawn(move || {
//!     clone2.post("http://example.domain/post").body("foo=bar").send().unwrap();
//! });
//! ```
use std::borrow::Cow;
use std::default::Default;
use std::io::{self, copy, Read};
use std::fmt;

use std::time::Duration;

use url::Url;
use url::ParseError as UrlError;

use header::{Headers, Header, HeaderFormat};
use header::{ContentLength, Host, Location};
use method::Method;
use net::{NetworkConnector, NetworkStream, SslClient};
use Error;

use self::proxy::{Proxy, tunnel};
use self::scheme::Scheme;
pub use self::pool::Pool;
pub use self::request::Request;
pub use self::response::Response;

mod proxy;
pub mod pool;
pub mod request;
pub mod response;

use http::Protocol;
use http::h1::Http11Protocol;


/// A Client to use additional features with Requests.
///
/// Clients can handle things such as: redirect policy, connection pooling.
pub struct Client {
    protocol: Box<Protocol + Send + Sync>,
    redirect_policy: RedirectPolicy,
    read_timeout: Option<Duration>,
    write_timeout: Option<Duration>,
    proxy: Option<(Scheme, Cow<'static, str>, u16)>
}

impl fmt::Debug for Client {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Client")
           .field("redirect_policy", &self.redirect_policy)
           .field("read_timeout", &self.read_timeout)
           .field("write_timeout", &self.write_timeout)
           .field("proxy", &self.proxy)
           .finish()
    }
}

impl Client {

    /// Create a new Client.
    pub fn new() -> Client {
        Client::with_pool_config(Default::default())
    }

    /// Create a new Client with a configured Pool Config.
    pub fn with_pool_config(config: pool::Config) -> Client {
        Client::with_connector(Pool::new(config))
    }

    /// Create a Client with an HTTP proxy to a (host, port).
    pub fn with_http_proxy<H>(host: H, port: u16) -> Client
    where H: Into<Cow<'static, str>> {
        let host = host.into();
        let proxy = tunnel((Scheme::Http, host.clone(), port));
        let mut client = Client::with_connector(Pool::with_connector(Default::default(), proxy));
        client.proxy = Some((Scheme::Http, host, port));
        client
    }

    /// Create a Client using a proxy with a custom connector and SSL client.
    pub fn with_proxy_config<C, S>(proxy_config: ProxyConfig<C, S>) -> Client
    where C: NetworkConnector + Send + Sync + 'static,
          C::Stream: NetworkStream + Send + Clone,
          S: SslClient<C::Stream> + Send + Sync + 'static {

        let scheme = proxy_config.scheme;
        let host = proxy_config.host;
        let port = proxy_config.port;
        let proxy = Proxy {
            proxy: (scheme.clone(), host.clone(), port),
            connector: proxy_config.connector,
            ssl: proxy_config.ssl,
        };

        let mut client = match proxy_config.pool_config {
            Some(pool_config) => Client::with_connector(Pool::with_connector(pool_config, proxy)),
            None => Client::with_connector(proxy),
        };
        client.proxy = Some((scheme, host, port));
        client
    }

    /// Create a new client with a specific connector.
    pub fn with_connector<C, S>(connector: C) -> Client
    where C: NetworkConnector<Stream=S> + Send + Sync + 'static, S: NetworkStream + Send {
        Client::with_protocol(Http11Protocol::with_connector(connector))
    }

    /// Create a new client with a specific `Protocol`.
    pub fn with_protocol<P: Protocol + Send + Sync + 'static>(protocol: P) -> Client {
        Client {
            protocol: Box::new(protocol),
            redirect_policy: Default::default(),
            read_timeout: None,
            write_timeout: None,
            proxy: None,
        }
    }

    /// Set the RedirectPolicy.
    pub fn set_redirect_policy(&mut self, policy: RedirectPolicy) {
        self.redirect_policy = policy;
    }

    /// Set the read timeout value for all requests.
    pub fn set_read_timeout(&mut self, dur: Option<Duration>) {
        self.read_timeout = dur;
    }

    /// Set the write timeout value for all requests.
    pub fn set_write_timeout(&mut self, dur: Option<Duration>) {
        self.write_timeout = dur;
    }

    /// Build a Get request.
    pub fn get<U: IntoUrl>(&self, url: U) -> RequestBuilder {
        self.request(Method::Get, url)
    }

    /// Build a Head request.
    pub fn head<U: IntoUrl>(&self, url: U) -> RequestBuilder {
        self.request(Method::Head, url)
    }

    /// Build a Patch request.
    pub fn patch<U: IntoUrl>(&self, url: U) -> RequestBuilder {
        self.request(Method::Patch, url)
    }

    /// Build a Post request.
    pub fn post<U: IntoUrl>(&self, url: U) -> RequestBuilder {
        self.request(Method::Post, url)
    }

    /// Build a Put request.
    pub fn put<U: IntoUrl>(&self, url: U) -> RequestBuilder {
        self.request(Method::Put, url)
    }

    /// Build a Delete request.
    pub fn delete<U: IntoUrl>(&self, url: U) -> RequestBuilder {
        self.request(Method::Delete, url)
    }


    /// Build a new request using this Client.
    pub fn request<U: IntoUrl>(&self, method: Method, url: U) -> RequestBuilder {
        RequestBuilder {
            client: self,
            method: method,
            url: url.into_url(),
            body: None,
            headers: None,
        }
    }
}

impl Default for Client {
    fn default() -> Client { Client::new() }
}

/// Options for an individual Request.
///
/// One of these will be built for you if you use one of the convenience
/// methods, such as `get()`, `post()`, etc.
pub struct RequestBuilder<'a> {
    client: &'a Client,
    // We store a result here because it's good to keep RequestBuilder
    // from being generic, but it is a nicer API to report the error
    // from `send` (when other errors may be happening, so it already
    // returns a `Result`). Why's it good to keep it non-generic? It
    // stops downstream crates having to remonomorphise and recompile
    // the code, which can take a while, since `send` is fairly large.
    // (For an extreme example, a tiny crate containing
    // `hyper::Client::new().get("x").send().unwrap();` took ~4s to
    // compile with a generic RequestBuilder, but 2s with this scheme,)
    url: Result<Url, UrlError>,
    headers: Option<Headers>,
    method: Method,
    body: Option<Body<'a>>,
}

impl<'a> RequestBuilder<'a> {

    /// Set a request body to be sent.
    pub fn body<B: Into<Body<'a>>>(mut self, body: B) -> RequestBuilder<'a> {
        self.body = Some(body.into());
        self
    }

    /// Add additional headers to the request.
    pub fn headers(mut self, headers: Headers) -> RequestBuilder<'a> {
        self.headers = Some(headers);
        self
    }

    /// Add an individual new header to the request.
    pub fn header<H: Header + HeaderFormat>(mut self, header: H) -> RequestBuilder<'a> {
        {
            let mut headers = match self.headers {
                Some(ref mut h) => h,
                None => {
                    self.headers = Some(Headers::new());
                    self.headers.as_mut().unwrap()
                }
            };

            headers.set(header);
        }
        self
    }

    /// Execute this request and receive a Response back.
    pub fn send(self) -> ::Result<Response> {
        let RequestBuilder { client, method, url, headers, body } = self;
        let mut url = try!(url);
        trace!("send method={:?}, url={:?}, client={:?}", method, url, client);

        let can_have_body = match method {
            Method::Get | Method::Head => false,
            _ => true
        };

        let mut body = if can_have_body {
            body
        } else {
            None
        };

        loop {
            let mut req = {
                let (host, port) = try!(get_host_and_port(&url));
                let mut message = try!(client.protocol.new_message(&host, port, url.scheme()));
                if url.scheme() == "http" && client.proxy.is_some() {
                    message.set_proxied(true);
                }

                let mut h = Headers::new();
                h.set(Host {
                    hostname: host.to_owned(),
                    port: Some(port),
                });
                if let Some(ref headers) = headers {
                    h.extend(headers.iter());
                }
                let headers = h;
                Request::with_headers_and_message(method.clone(), url.clone(), headers, message)
            };

            try!(req.set_write_timeout(client.write_timeout));
            try!(req.set_read_timeout(client.read_timeout));

            match (can_have_body, body.as_ref()) {
                (true, Some(body)) => match body.size() {
                    Some(size) => req.headers_mut().set(ContentLength(size)),
                    None => (), // chunked, Request will add it automatically
                },
                (true, None) => req.headers_mut().set(ContentLength(0)),
                _ => () // neither
            }
            let mut streaming = try!(req.start());
            body.take().map(|mut rdr| copy(&mut rdr, &mut streaming));
            let res = try!(streaming.send());
            if !res.status.is_redirection() {
                return Ok(res)
            }
            debug!("redirect code {:?} for {}", res.status, url);

            let loc = {
                // punching borrowck here
                let loc = match res.headers.get::<Location>() {
                    Some(&Location(ref loc)) => {
                        Some(url.join(loc))
                    }
                    None => {
                        debug!("no Location header");
                        // could be 304 Not Modified?
                        None
                    }
                };
                match loc {
                    Some(r) => r,
                    None => return Ok(res)
                }
            };
            url = match loc {
                Ok(u) => u,
                Err(e) => {
                    debug!("Location header had invalid URI: {:?}", e);
                    return Ok(res);
                }
            };
            match client.redirect_policy {
                // separate branches because they can't be one
                RedirectPolicy::FollowAll => (), //continue
                RedirectPolicy::FollowIf(cond) if cond(&url) => (), //continue
                _ => return Ok(res),
            }
        }
    }
}

/// An enum of possible body types for a Request.
pub enum Body<'a> {
    /// A Reader does not necessarily know it's size, so it is chunked.
    ChunkedBody(&'a mut (Read + 'a)),
    /// For Readers that can know their size, like a `File`.
    SizedBody(&'a mut (Read + 'a), u64),
    /// A String has a size, and uses Content-Length.
    BufBody(&'a [u8] , usize),
}

impl<'a> Body<'a> {
    fn size(&self) -> Option<u64> {
        match *self {
            Body::SizedBody(_, len) => Some(len),
            Body::BufBody(_, len) => Some(len as u64),
            _ => None
        }
    }
}

impl<'a> Read for Body<'a> {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match *self {
            Body::ChunkedBody(ref mut r) => r.read(buf),
            Body::SizedBody(ref mut r, _) => r.read(buf),
            Body::BufBody(ref mut r, _) => Read::read(r, buf),
        }
    }
}

impl<'a> Into<Body<'a>> for &'a [u8] {
    #[inline]
    fn into(self) -> Body<'a> {
        Body::BufBody(self, self.len())
    }
}

impl<'a> Into<Body<'a>> for &'a str {
    #[inline]
    fn into(self) -> Body<'a> {
        self.as_bytes().into()
    }
}

impl<'a> Into<Body<'a>> for &'a String {
    #[inline]
    fn into(self) -> Body<'a> {
        self.as_bytes().into()
    }
}

impl<'a, R: Read> From<&'a mut R> for Body<'a> {
    #[inline]
    fn from(r: &'a mut R) -> Body<'a> {
        Body::ChunkedBody(r)
    }
}

/// A helper trait to convert common objects into a Url.
pub trait IntoUrl {
    /// Consumes the object, trying to return a Url.
    fn into_url(self) -> Result<Url, UrlError>;
}

impl IntoUrl for Url {
    fn into_url(self) -> Result<Url, UrlError> {
        Ok(self)
    }
}

impl<'a> IntoUrl for &'a str {
    fn into_url(self) -> Result<Url, UrlError> {
        Url::parse(self)
    }
}

impl<'a> IntoUrl for &'a String {
    fn into_url(self) -> Result<Url, UrlError> {
        Url::parse(self)
    }
}

/// Proxy server configuration with a custom connector and TLS wrapper.
pub struct ProxyConfig<C, S>
where C: NetworkConnector + Send + Sync + 'static,
      C::Stream: NetworkStream + Clone + Send,
      S: SslClient<C::Stream> + Send + Sync + 'static {
    scheme: Scheme,
    host: Cow<'static, str>,
    port: u16,
    pool_config: Option<pool::Config>,
    connector: C,
    ssl: S,
}

impl<C, S> ProxyConfig<C, S>
where C: NetworkConnector + Send + Sync + 'static,
      C::Stream: NetworkStream + Clone + Send,
      S: SslClient<C::Stream> + Send + Sync + 'static {

    /// Create a new `ProxyConfig`.
    #[inline]
    pub fn new<H: Into<Cow<'static, str>>>(scheme: &str, host: H, port: u16, connector: C, ssl: S) -> ProxyConfig<C, S> {
        ProxyConfig {
            scheme: scheme.into(),
            host: host.into(),
            port: port,
            pool_config: Some(pool::Config::default()),
            connector: connector,
            ssl: ssl,
        }
    }

    /// Change the `pool::Config` for the proxy.
    ///
    /// Passing `None` disables the `Pool`.
    ///
    /// The default is enabled, with the default `pool::Config`.
    pub fn set_pool_config(&mut self, pool_config: Option<pool::Config>) {
        self.pool_config = pool_config;
    }
}

/// Behavior regarding how to handle redirects within a Client.
#[derive(Copy)]
pub enum RedirectPolicy {
    /// Don't follow any redirects.
    FollowNone,
    /// Follow all redirects.
    FollowAll,
    /// Follow a redirect if the contained function returns true.
    FollowIf(fn(&Url) -> bool),
}

impl fmt::Debug for RedirectPolicy {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            RedirectPolicy::FollowNone => fmt.write_str("FollowNone"),
            RedirectPolicy::FollowAll => fmt.write_str("FollowAll"),
            RedirectPolicy::FollowIf(_) => fmt.write_str("FollowIf"),
        }
    }
}

// This is a hack because of upstream typesystem issues.
impl Clone for RedirectPolicy {
    fn clone(&self) -> RedirectPolicy {
        *self
    }
}

impl Default for RedirectPolicy {
    fn default() -> RedirectPolicy {
        RedirectPolicy::FollowAll
    }
}


fn get_host_and_port(url: &Url) -> ::Result<(&str, u16)> {
    let host = match url.host_str() {
        Some(host) => host,
        None => return Err(Error::Uri(UrlError::EmptyHost)),
    };
    trace!("host={:?}", host);
    let port = match url.port_or_known_default() {
        Some(port) => port,
        None => return Err(Error::Uri(UrlError::InvalidPort)),
    };
    trace!("port={:?}", port);
    Ok((host, port))
}

mod scheme {

    #[derive(Clone, PartialEq, Eq, Debug, Hash)]
    pub enum Scheme {
        Http,
        Https,
        Other(String),
    }

    impl<'a> From<&'a str> for Scheme {
        fn from(s: &'a str) -> Scheme {
            match s {
                "http" => Scheme::Http,
                "https" => Scheme::Https,
                s => Scheme::Other(String::from(s)),
            }
        }
    }

    impl AsRef<str> for Scheme {
        fn as_ref(&self) -> &str {
            match *self {
                Scheme::Http => "http",
                Scheme::Https => "https",
                Scheme::Other(ref s) => s,
            }
        }
    }

}

#[cfg(test)]
mod tests {
    use std::io::Read;
    use header::Server;
    use http::h1::Http11Message;
    use mock::{MockStream, MockSsl};
    use super::{Client, RedirectPolicy};
    use super::scheme::Scheme;
    use super::proxy::Proxy;
    use super::pool::Pool;
    use url::Url;

    mock_connector!(MockRedirectPolicy {
        "http://127.0.0.1" =>       "HTTP/1.1 301 Redirect\r\n\
                                     Location: http://127.0.0.2\r\n\
                                     Server: mock1\r\n\
                                     \r\n\
                                    "
        "http://127.0.0.2" =>       "HTTP/1.1 302 Found\r\n\
                                     Location: https://127.0.0.3\r\n\
                                     Server: mock2\r\n\
                                     \r\n\
                                    "
        "https://127.0.0.3" =>      "HTTP/1.1 200 OK\r\n\
                                     Server: mock3\r\n\
                                     \r\n\
                                    "
    });


    #[test]
    fn test_proxy() {
        use super::pool::PooledStream;
        type MessageStream = PooledStream<super::proxy::Proxied<MockStream, MockStream>>;
        mock_connector!(ProxyConnector {
            b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n"
        });
        let tunnel = Proxy {
            connector: ProxyConnector,
            proxy: (Scheme::Http, "example.proxy".into(), 8008),
            ssl: MockSsl,
        };
        let mut client = Client::with_connector(Pool::with_connector(Default::default(), tunnel));
        client.proxy = Some((Scheme::Http, "example.proxy".into(), 8008));
        let mut dump = vec![];
        client.get("http://127.0.0.1/foo/bar").send().unwrap().read_to_end(&mut dump).unwrap();

        let box_message = client.protocol.new_message("127.0.0.1", 80, "http").unwrap();
        let message = box_message.downcast::<Http11Message>().unwrap();
        let stream =  message.into_inner().downcast::<MessageStream>().unwrap().into_inner().into_normal().unwrap();

        let s = ::std::str::from_utf8(&stream.write).unwrap();
        let request_line = "GET http://127.0.0.1/foo/bar HTTP/1.1\r\n";
        assert!(s.starts_with(request_line), "{:?} doesn't start with {:?}", s, request_line);
        assert!(s.contains("Host: 127.0.0.1\r\n"));
    }

    #[test]
    fn test_proxy_tunnel() {
        use super::pool::PooledStream;
        type MessageStream = PooledStream<super::proxy::Proxied<MockStream, MockStream>>;

        mock_connector!(ProxyConnector {
            b"HTTP/1.1 200 OK\r\n\r\n",
            b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n"
        });
        let tunnel = Proxy {
            connector: ProxyConnector,
            proxy: (Scheme::Http, "example.proxy".into(), 8008),
            ssl: MockSsl,
        };
        let mut client = Client::with_connector(Pool::with_connector(Default::default(), tunnel));
        client.proxy = Some((Scheme::Http, "example.proxy".into(), 8008));
        let mut dump = vec![];
        client.get("https://127.0.0.1/foo/bar").send().unwrap().read_to_end(&mut dump).unwrap();

        let box_message = client.protocol.new_message("127.0.0.1", 443, "https").unwrap();
        let message = box_message.downcast::<Http11Message>().unwrap();
        let stream = message.into_inner().downcast::<MessageStream>().unwrap().into_inner().into_tunneled().unwrap();

        let s = ::std::str::from_utf8(&stream.write).unwrap();
        let connect_line = "CONNECT 127.0.0.1:443 HTTP/1.1\r\nHost: 127.0.0.1:443\r\n\r\n";
        assert_eq!(&s[..connect_line.len()], connect_line);

        let s = &s[connect_line.len()..];
        let request_line = "GET /foo/bar HTTP/1.1\r\n";
        assert_eq!(&s[..request_line.len()], request_line);
        assert!(s.contains("Host: 127.0.0.1\r\n"));
    }

    #[test]
    fn test_redirect_followall() {
        let mut client = Client::with_connector(MockRedirectPolicy);
        client.set_redirect_policy(RedirectPolicy::FollowAll);

        let res = client.get("http://127.0.0.1").send().unwrap();
        assert_eq!(res.headers.get(), Some(&Server("mock3".to_owned())));
    }

    #[test]
    fn test_redirect_dontfollow() {
        let mut client = Client::with_connector(MockRedirectPolicy);
        client.set_redirect_policy(RedirectPolicy::FollowNone);
        let res = client.get("http://127.0.0.1").send().unwrap();
        assert_eq!(res.headers.get(), Some(&Server("mock1".to_owned())));
    }

    #[test]
    fn test_redirect_followif() {
        fn follow_if(url: &Url) -> bool {
            !url.as_str().contains("127.0.0.3")
        }
        let mut client = Client::with_connector(MockRedirectPolicy);
        client.set_redirect_policy(RedirectPolicy::FollowIf(follow_if));
        let res = client.get("http://127.0.0.1").send().unwrap();
        assert_eq!(res.headers.get(), Some(&Server("mock2".to_owned())));
    }

    mock_connector!(Issue640Connector {
        b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\n",
        b"GET",
        b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\n",
        b"HEAD",
        b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\n",
        b"POST"
    });

    // see issue #640
    #[test]
    fn test_head_response_body_keep_alive() {
        let client = Client::with_connector(Pool::with_connector(Default::default(), Issue640Connector));

        let mut s = String::new();
        client.get("http://127.0.0.1").send().unwrap().read_to_string(&mut s).unwrap();
        assert_eq!(s, "GET");

        let mut s = String::new();
        client.head("http://127.0.0.1").send().unwrap().read_to_string(&mut s).unwrap();
        assert_eq!(s, "");

        let mut s = String::new();
        client.post("http://127.0.0.1").send().unwrap().read_to_string(&mut s).unwrap();
        assert_eq!(s, "POST");
    }
}
