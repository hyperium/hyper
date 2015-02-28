//! HTTP Client
//!
//! # Usage
//!
//! The `Client` API is designed for most people to make HTTP requests.
//! It utilizes the lower level `Request` API.
//!
//! ```no_run
//! use hyper::Client;
//!
//! let mut client = Client::new();
//!
//! let mut res = client.get("http://example.domain").send().unwrap();
//! assert_eq!(res.status, hyper::Ok);
//! ```
//!
//! The returned value from is a `Response`, which provides easy access
//! to the `status`, the `headers`, and the response body via the `Writer`
//! trait.
extern crate rand;
use self::rand::Rng;

use std::default::Default;
use std::old_io::IoResult;
use std::old_io::util::copy;
use std::iter::Extend;

use url::UrlParser;
use url::ParseError as UrlError;

use header::{Headers, Header, HeaderFormat, SetCookie};
use header::Cookie;
use header::{ContentLength, Location};
use method::Method;
use net::{NetworkConnector, HttpConnector, ContextVerifier};
use status::StatusClass::Redirection;
use {Url, Port, HttpResult};
use HttpError::HttpUriError;

use cookie::Cookie as CookiePair;
use cookie::CookieJar;

pub use self::request::Request;
pub use self::response::Response;

pub mod request;
pub mod response;

/// A Client to use additional features with Requests.
///
/// Clients can handle things such as: redirect policy.
pub struct Client<C> {
    connector: C,
    cookies_policy: CookiesPolicy,
    redirect_policy: RedirectPolicy,
}

impl<'v> Client<HttpConnector<'v>> {

    /// Create a new Client.
    pub fn new() -> Client<HttpConnector<'v>> {
        Client::with_connector(HttpConnector(None))
    }

    /// Set the SSL verifier callback for use with OpenSSL.
    pub fn set_ssl_verifier(&mut self, verifier: ContextVerifier<'v>) {
        self.connector = HttpConnector(Some(verifier));
    }

}

impl<C: NetworkConnector> Client<C> {

    /// Create a new client with a specific connector.
    pub fn with_connector(connector: C) -> Client<C> {
        Client {
            connector: connector,
            cookies_policy: Default::default(),
            redirect_policy: Default::default()
        }
    }

    /// Set the RedirectPolicy.
    pub fn set_redirect_policy(&mut self, policy: RedirectPolicy) {
        self.redirect_policy = policy;
    }

    /// Set the CookiesPolicy
    pub fn set_cookies_policy(&mut self, policy: CookiesPolicy) {
        self.cookies_policy = policy;
    }

    /// Execute a Get request.
    pub fn get<U: IntoUrl>(&mut self, url: U) -> RequestBuilder<U, C> {
        self.request(Method::Get, url)
    }

    /// Execute a Head request.
    pub fn head<U: IntoUrl>(&mut self, url: U) -> RequestBuilder<U, C> {
        self.request(Method::Head, url)
    }

    /// Execute a Post request.
    pub fn post<U: IntoUrl>(&mut self, url: U) -> RequestBuilder<U, C> {
        self.request(Method::Post, url)
    }

    /// Execute a Put request.
    pub fn put<U: IntoUrl>(&mut self, url: U) -> RequestBuilder<U, C> {
        self.request(Method::Put, url)
    }

    /// Execute a Delete request.
    pub fn delete<U: IntoUrl>(&mut self, url: U) -> RequestBuilder<U, C> {
        self.request(Method::Delete, url)
    }


    /// Build a new request using this Client.
    pub fn request<U: IntoUrl>(&mut self, method: Method, url: U) -> RequestBuilder<U, C> {
        RequestBuilder {
            client: self,
            method: method,
            url: url,
            body: None,
            // because sometimes maybe I will do different ways.
            headers: None,
        }
    }
}

/// Options for an individual Request.
///
/// One of these will be built for you if you use one of the convenience
/// methods, such as `get()`, `post()`, etc.
pub struct RequestBuilder<'a, U: IntoUrl, C: NetworkConnector + 'a> {
    client: &'a mut Client<C>,
    url: U,
    headers: Option<Headers>,
    method: Method,
    body: Option<Body<'a>>,
}

impl<'a, U: IntoUrl, C: NetworkConnector> RequestBuilder<'a, U, C> {

    /// Set a request body to be sent.
    pub fn body<B: IntoBody<'a>>(mut self, body: B) -> RequestBuilder<'a, U, C> {
        self.body = Some(body.into_body());
        self
    }

    /// Add additional headers to the request.
    pub fn headers(mut self, headers: Headers) -> RequestBuilder<'a, U, C> {
        self.headers = Some(headers);
        self
    }

    /// Add an individual new header to the request.
    pub fn header<H: Header + HeaderFormat>(mut self, header: H) -> RequestBuilder<'a, U, C> {
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
        let RequestBuilder { client, method, url, mut headers, body } = self;
        let mut url = try!(url.into_url());
        debug!("client.request {:?} {:?}", method, url);

        let can_have_body = match &method {
            &Method::Get | &Method::Head => false,
            _ => true
        };

        let mut body = if can_have_body {
            body.map(|b| b.into_body())
        } else {
             None
        };
        let mut _v;
        let mut cookiejar = CookieJar::new({
            _v = vec![0u8; 32];
            rand::thread_rng().fill_bytes(&mut _v);
            &mut _v
        });

        headers.as_ref().map( |headers| {
            match headers.get::<Cookie>() {
                Some(&Cookie(ref cookies)) => {
                    for cookie in cookies.iter() {
                        cookiejar.add(cookie.clone());
                    }
                },
                None => ()
            }
        });

        loop {
            let mut req = try!(Request::with_connector(method.clone(), url.clone(), &mut client.connector));
            headers.as_mut().map(|headers| headers.set(Cookie::from_cookie_jar(&cookiejar)));
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

            match res.headers.get::<SetCookie>() {
                Some(setcookie) => setcookie.apply_to_cookie_jar(&mut cookiejar),
                None => {
                    debug!("no Set-Header header");
                }   
            };

            if res.status.class() != Redirection {
                return Ok(res)
            }

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
    ChunkedBody(&'a mut (Reader + 'a)),
    /// For Readers that can know their size, like a `File`.
    SizedBody(&'a mut (Reader + 'a), u64),
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

impl<'a> Reader for Body<'a> {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        match *self {
            Body::ChunkedBody(ref mut r) => r.read(buf),
            Body::SizedBody(ref mut r, _) => r.read(buf),
            Body::BufBody(ref mut r, _) => r.read(buf),
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

impl<'a, R: Reader> IntoBody<'a> for &'a mut R {
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

/// FIXME
#[derive(Copy)]
pub enum CookiesPolicy {
    /// Accept all cookies.
    AcceptAll,
    /// Don't accept cookies.
    AcceptNone,
    /// Accept cookies the contained function returns.
    AcceptSome(fn(&Vec<CookiePair>) -> &Vec<CookiePair>),
}

impl Clone for CookiesPolicy {
    fn clone(&self) -> CookiesPolicy {
        *self 
    }
}

impl Default for CookiesPolicy {
    fn default() -> CookiesPolicy {
        CookiesPolicy::AcceptAll
    }
}


fn get_host_and_port(url: &Url) -> HttpResult<(String, Port)> {
    let host = match url.serialize_host() {
        Some(host) => host,
        None => return Err(HttpUriError(UrlError::EmptyHost))
    };
    debug!("host={:?}", host);
    let port = match url.port_or_default() {
        Some(port) => port,
        None => return Err(HttpUriError(UrlError::InvalidPort))
    };
    debug!("port={:?}", port);
    Ok((host, port))
}

#[cfg(test)]
mod tests {
    extern crate rand;
    use self::rand::Rng;

    use header::Server;
    use header::Cookie;
    use cookie::Cookie as CookiePair;
    use super::{Client, RedirectPolicy, CookiesPolicy};
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

    mock_connector!(MockCookiesPolicy {
        "http://127.0.0.1" =>       "HTTP/1.1 301 Redirect\r\n\
                                     Location: http://127.0.0.2\r\n\
                                     Server: mock1\r\n\
                                     Set-Cookie: k2=v2; Path=/\r\n\
                                     Set-Cookie: k1=v1; Path=/\r\n\
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
    fn test_cookies_acceptall() {
        fn follow_if(url: &Url) -> bool {
            !url.serialize().contains("127.0.0.3")
        }
        let mut client = Client::with_connector(MockCookiesPolicy);
        client.set_redirect_policy(RedirectPolicy::FollowAll(follow_if));
        client.set_cookies_policy(CookiesPolicy::AcceptAll);

        let res = client.get("http://127.0.0.1").send().unwrap();
        assert_eq!(res.headers.get(), Some(&Server("mock2".to_string())));
        let mut _v;
        println!("{:?}", res.headers);
        let cookiejar = res.headers.get::<Cookie>().unwrap().to_cookie_jar({
            _v = vec![0u8; 32];
            rand::thread_rng().fill_bytes(&mut _v);
            &mut _v
        });
        assert_eq!(cookiejar.find("k2"), Some(CookiePair::new("k2".to_string(), "v2".to_string())));

    }

}
