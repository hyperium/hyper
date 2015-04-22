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
//! let mut client = Client::new();
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
//! let mut client = Client::new();
//!
//! let res = client.post("http://exmaple.domain")
//!     .body("foo=bar")
//!     .send()
//!     .unwrap();
//! assert_eq!(res.status, hyper::Ok);
//! ```
use std::default::Default;
use std::io::{self, copy, Read};
use std::iter::Extend;

use url::UrlParser;
use url::ParseError as UrlError;

use header::{Headers, Header, HeaderFormat};
use header::{ContentLength, Location};
use method::Method;
use net::{NetworkConnector, NetworkStream, HttpConnector, ContextVerifier};
use status::StatusClass::Redirection;
use {Url, HttpResult};
use HttpError::HttpUriError;

pub use self::request::Request;
pub use self::response::Response;

pub mod request;
pub mod response;

/// A Client to use additional features with Requests.
///
/// Clients can handle things such as: redirect policy.
pub struct Client {
    connector: Connector,
    redirect_policy: RedirectPolicy,
}

impl Client {

    /// Create a new Client.
    pub fn new() -> Client {
        Client::with_connector(HttpConnector(None))
    }

    /// Create a new client with a specific connector.
    pub fn with_connector<C, S>(connector: C) -> Client
    where C: NetworkConnector<Stream=S> + Send + 'static, S: NetworkStream + Send {
        Client {
            connector: with_connector(connector),
            redirect_policy: Default::default()
        }
    }

    /// Set the SSL verifier callback for use with OpenSSL.
    pub fn set_ssl_verifier(&mut self, verifier: ContextVerifier) {
        self.connector = with_connector(HttpConnector(Some(verifier)));
    }

    /// Set the RedirectPolicy.
    pub fn set_redirect_policy(&mut self, policy: RedirectPolicy) {
        self.redirect_policy = policy;
    }

    /// Build a Get request.
    pub fn get<U: IntoUrl>(&mut self, url: U) -> RequestBuilder<U> {
        self.request(Method::Get, url)
    }

    /// Build a Head request.
    pub fn head<U: IntoUrl>(&mut self, url: U) -> RequestBuilder<U> {
        self.request(Method::Head, url)
    }

    /// Build a Post request.
    pub fn post<U: IntoUrl>(&mut self, url: U) -> RequestBuilder<U> {
        self.request(Method::Post, url)
    }

    /// Build a Put request.
    pub fn put<U: IntoUrl>(&mut self, url: U) -> RequestBuilder<U> {
        self.request(Method::Put, url)
    }

    /// Build a Delete request.
    pub fn delete<U: IntoUrl>(&mut self, url: U) -> RequestBuilder<U> {
        self.request(Method::Delete, url)
    }


    /// Build a new request using this Client.
    pub fn request<U: IntoUrl>(&mut self, method: Method, url: U) -> RequestBuilder<U> {
        RequestBuilder {
            client: self,
            method: method,
            url: url,
            body: None,
            headers: None,
        }
    }
}

fn with_connector<C: NetworkConnector<Stream=S> + Send + 'static, S: NetworkStream + Send>(c: C) -> Connector {
    Connector(Box::new(ConnAdapter(c)))
}

struct ConnAdapter<C: NetworkConnector + Send>(C);

impl<C: NetworkConnector<Stream=S> + Send, S: NetworkStream + Send> NetworkConnector for ConnAdapter<C> {
    type Stream = Box<NetworkStream + Send>;
    #[inline]
    fn connect(&mut self, host: &str, port: u16, scheme: &str)
        -> io::Result<Box<NetworkStream + Send>> {
        Ok(try!(self.0.connect(host, port, scheme)).into())
    }
}

struct Connector(Box<NetworkConnector<Stream=Box<NetworkStream + Send>> + Send>);

impl NetworkConnector for Connector {
    type Stream = Box<NetworkStream + Send>;
    #[inline]
    fn connect(&mut self, host: &str, port: u16, scheme: &str)
        -> io::Result<Box<NetworkStream + Send>> {
        Ok(try!(self.0.connect(host, port, scheme)).into())
    }
}

/// Options for an individual Request.
///
/// One of these will be built for you if you use one of the convenience
/// methods, such as `get()`, `post()`, etc.
pub struct RequestBuilder<'a, U: IntoUrl> {
    client: &'a mut Client,
    url: U,
    headers: Option<Headers>,
    method: Method,
    body: Option<Body<'a>>,
}

impl<'a, U: IntoUrl> RequestBuilder<'a, U> {

    /// Set a request body to be sent.
    pub fn body<B: IntoBody<'a>>(mut self, body: B) -> RequestBuilder<'a, U> {
        self.body = Some(body.into_body());
        self
    }

    /// Add additional headers to the request.
    pub fn headers(mut self, headers: Headers) -> RequestBuilder<'a, U> {
        self.headers = Some(headers);
        self
    }

    /// Add an individual new header to the request.
    pub fn header<H: Header + HeaderFormat>(mut self, header: H) -> RequestBuilder<'a, U> {
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
    pub fn send(self) -> HttpResult<Response> {
        let RequestBuilder { client, method, url, headers, body } = self;
        let mut url = try!(url.into_url());
        trace!("send {:?} {:?}", method, url);

        let can_have_body = match &method {
            &Method::Get | &Method::Head => false,
            _ => true
        };

        let mut body = if can_have_body {
            body.map(|b| b.into_body())
        } else {
             None
        };

        loop {
            let mut req = try!(Request::with_connector(method.clone(), url.clone(), &mut client.connector));
            headers.as_ref().map(|headers| req.headers_mut().extend(headers.iter()));

            match (can_have_body, body.as_ref()) {
                (true, Some(ref body)) => match body.size() {
                    Some(size) => req.headers_mut().set(ContentLength(size)),
                    None => (), // chunked, Request will add it automatically
                },
                (true, None) => req.headers_mut().set(ContentLength(0)),
                _ => () // neither
            }
            let mut streaming = try!(req.start());
            body.take().map(|mut rdr| copy(&mut rdr, &mut streaming));
            let res = try!(streaming.send());
            if res.status.class() != Redirection {
                return Ok(res)
            }
            debug!("redirect code {:?} for {:?}", res.status, url);

            let loc = {
                // punching borrowck here
                let loc = match res.headers.get::<Location>() {
                    Some(&Location(ref loc)) => {
                        Some(UrlParser::new().base_url(&url).parse(&loc[..]))
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
                Ok(u) => {
                    inspect!("Location", u)
                },
                Err(e) => {
                    debug!("Location header had invalid URI: {:?}", e);
                    return Ok(res);
                }
            };
            match client.redirect_policy {
                // separate branches because they cant be one
                RedirectPolicy::FollowAll => (), //continue
                RedirectPolicy::FollowIf(cond) if cond(&url) => (), //continue
                _ => return Ok(res),
            }
        }
    }
}

/// A helper trait to allow overloading of the body parameter.
pub trait IntoBody<'a> {
    /// Consumes self into an instance of `Body`.
    fn into_body(self) -> Body<'a>;
}

/// The target enum for the IntoBody trait.
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

// To allow someone to pass a `Body::SizedBody()` themselves.
impl<'a> IntoBody<'a> for Body<'a> {
    #[inline]
    fn into_body(self) -> Body<'a> {
        self
    }
}

impl<'a> IntoBody<'a> for &'a [u8] {
    #[inline]
    fn into_body(self) -> Body<'a> {
        Body::BufBody(self, self.len())
    }
}

impl<'a> IntoBody<'a> for &'a str {
    #[inline]
    fn into_body(self) -> Body<'a> {
        self.as_bytes().into_body()
    }
}

impl<'a, R: Read> IntoBody<'a> for &'a mut R {
    #[inline]
    fn into_body(self) -> Body<'a> {
        Body::ChunkedBody(self)
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

fn get_host_and_port(url: &Url) -> HttpResult<(String, u16)> {
    let host = match url.serialize_host() {
        Some(host) => host,
        None => return Err(HttpUriError(UrlError::EmptyHost))
    };
    trace!("host={:?}", host);
    let port = match url.port_or_default() {
        Some(port) => port,
        None => return Err(HttpUriError(UrlError::InvalidPort))
    };
    trace!("port={:?}", port);
    Ok((host, port))
}

#[cfg(test)]
mod tests {
    use header::Server;
    use super::{Client, RedirectPolicy};
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
    fn test_redirect_followall() {
        let mut client = Client::with_connector(MockRedirectPolicy);
        client.set_redirect_policy(RedirectPolicy::FollowAll);

        let res = client.get("http://127.0.0.1").send().unwrap();
        assert_eq!(res.headers.get(), Some(&Server("mock3".to_string())));
    }

    #[test]
    fn test_redirect_dontfollow() {
        let mut client = Client::with_connector(MockRedirectPolicy);
        client.set_redirect_policy(RedirectPolicy::FollowNone);
        let res = client.get("http://127.0.0.1").send().unwrap();
        assert_eq!(res.headers.get(), Some(&Server("mock1".to_string())));
    }

    #[test]
    fn test_redirect_followif() {
        fn follow_if(url: &Url) -> bool {
            !url.serialize().contains("127.0.0.3")
        }
        let mut client = Client::with_connector(MockRedirectPolicy);
        client.set_redirect_policy(RedirectPolicy::FollowIf(follow_if));
        let res = client.get("http://127.0.0.1").send().unwrap();
        assert_eq!(res.headers.get(), Some(&Server("mock2".to_string())));
    }

}
