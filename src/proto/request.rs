use std::fmt;
#[cfg(feature = "compat")]
use std::mem::replace;
use std::net::SocketAddr;

#[cfg(feature = "compat")]
use http;

use header::Headers;
use proto::{Body, MessageHead, RequestHead, RequestLine};
use method::Method;
use uri::{self, Uri};
use version::HttpVersion;

/// An HTTP Request
pub struct Request<B = Body> {
    method: Method,
    uri: Uri,
    version: HttpVersion,
    headers: Headers,
    body: Option<B>,
    is_proxy: bool,
    remote_addr: Option<SocketAddr>,
}

impl<B> Request<B> {
    /// Construct a new Request.
    #[inline]
    pub fn new(method: Method, uri: Uri) -> Request<B> {
        Request {
            method: method,
            uri: uri,
            version: HttpVersion::default(),
            headers: Headers::new(),
            body: None,
            is_proxy: false,
            remote_addr: None,
        }
    }

    /// Read the Request Uri.
    #[inline]
    pub fn uri(&self) -> &Uri { &self.uri }

    /// Read the Request Version.
    #[inline]
    pub fn version(&self) -> HttpVersion { self.version }

    /// Read the Request headers.
    #[inline]
    pub fn headers(&self) -> &Headers { &self.headers }

    /// Read the Request method.
    #[inline]
    pub fn method(&self) -> &Method { &self.method }

    /// Read the Request body.
    #[inline]
    pub fn body_ref(&self) -> Option<&B> { self.body.as_ref() }

    /// The remote socket address of this request
    ///
    /// This is an `Option`, because some underlying transports may not have
    /// a socket address, such as Unix Sockets.
    ///
    /// This field is not used for outgoing requests.
    #[inline]
    pub fn remote_addr(&self) -> Option<SocketAddr> { self.remote_addr }

    /// The target path of this Request.
    #[inline]
    pub fn path(&self) -> &str {
        self.uri.path()
    }

    /// The query string of this Request.
    #[inline]
    pub fn query(&self) -> Option<&str> {
        self.uri.query()
    }

    /// Set the Method of this request.
    #[inline]
    pub fn set_method(&mut self, method: Method) { self.method = method; }

    /// Get a mutable reference to the Request headers.
    #[inline]
    pub fn headers_mut(&mut self) -> &mut Headers { &mut self.headers }

    /// Set the `Uri` of this request.
    #[inline]
    pub fn set_uri(&mut self, uri: Uri) { self.uri = uri; }

    /// Set the `HttpVersion` of this request.
    #[inline]
    pub fn set_version(&mut self, version: HttpVersion) { self.version = version; }

    /// Set the body of the request.
    ///
    /// By default, the body will be sent using `Transfer-Encoding: chunked`. To
    /// override this behavior, manually set a [`ContentLength`] header with the
    /// length of `body`.
    #[inline]
    pub fn set_body<T: Into<B>>(&mut self, body: T) { self.body = Some(body.into()); }

    /// Set that the URI should use the absolute form.
    ///
    /// This is only needed when talking to HTTP/1 proxies to URLs not
    /// protected by TLS.
    #[inline]
    pub fn set_proxy(&mut self, is_proxy: bool) { self.is_proxy = is_proxy; }
}

impl Request<Body> {
    /// Deconstruct this Request into its pieces.
    ///
    /// Modifying these pieces will have no effect on how hyper behaves.
    #[inline]
    pub fn deconstruct(self) -> (Method, Uri, HttpVersion, Headers, Body) {
        (self.method, self.uri, self.version, self.headers, self.body.unwrap_or_default())
    }

    /// Take the Request body.
    #[inline]
    pub fn body(self) -> Body { self.body.unwrap_or_default() }
}

impl<B> fmt::Debug for Request<B> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Request")
            .field("method", &self.method)
            .field("uri", &self.uri)
            .field("version", &self.version)
            .field("remote_addr", &self.remote_addr)
            .field("headers", &self.headers)
            .finish()
    }
}

#[cfg(feature = "compat")]
impl From<Request> for http::Request<Body> {
    fn from(from_req: Request) -> http::Request<Body> {
        let (m, u, v, h, b) = from_req.deconstruct();

        let to_req = http::Request::new(());
        let (mut to_parts, _) = to_req.into_parts();

        to_parts.method = m.into();
        to_parts.uri = u.into();
        to_parts.version = v.into();
        to_parts.headers = h.into();

        http::Request::from_parts(to_parts, b)
    }
}

#[cfg(feature = "compat")]
impl<B> From<http::Request<B>> for Request<B> {
    fn from(from_req: http::Request<B>) -> Request<B> {
        let (from_parts, body) = from_req.into_parts();

        let mut to_req = Request::new(from_parts.method.into(), from_parts.uri.into());
        to_req.set_version(from_parts.version.into());
        replace(to_req.headers_mut(), from_parts.headers.into());
        to_req.set_body(body);
        to_req
    }
}

/// Constructs a request using a received ResponseHead and optional body
pub fn from_wire(addr: Option<SocketAddr>, incoming: RequestHead, body: Option<Body>) -> Request<Body> {
    let MessageHead { version, subject: RequestLine(method, uri), headers } = incoming;

    Request {
        method: method,
        uri: uri,
        headers: headers,
        version: version,
        remote_addr: addr,
        body: body,
        is_proxy: false,
    }
}

pub fn split<B>(req: Request<B>) -> (RequestHead, Option<B>) {
    let uri = if req.is_proxy {
        req.uri
    } else {
        uri::origin_form(&req.uri)
    };
    let head = RequestHead {
        subject: ::proto::RequestLine(req.method, uri),
        headers: req.headers,
        version: req.version,
    };
    (head, req.body)
}

#[cfg(test)]
mod tests {
    /*
    use std::io::Write;
    use std::str::from_utf8;
    use Url;
    use method::Method::{Get, Head, Post};
    use mock::{MockStream, MockConnector};
    use net::Fresh;
    use header::{ContentLength,TransferEncoding,Encoding};
    use url::form_urlencoded;
    use super::Request;
    use http::h1::Http11Message;

    fn run_request(req: Request<Fresh>) -> Vec<u8> {
        let req = req.start().unwrap();
        let message = req.message;
        let mut message = message.downcast::<Http11Message>().ok().unwrap();
        message.flush_outgoing().unwrap();
        let stream = *message
            .into_inner().downcast::<MockStream>().ok().unwrap();
        stream.write
    }

    fn assert_no_body(s: &str) {
        assert!(!s.contains("Content-Length:"));
        assert!(!s.contains("Transfer-Encoding:"));
    }

    #[test]
    fn test_get_empty_body() {
        let req = Request::with_connector(
            Get, Url::parse("http://example.dom").unwrap(), &mut MockConnector
        ).unwrap();
        let bytes = run_request(req);
        let s = from_utf8(&bytes[..]).unwrap();
        assert_no_body(s);
    }

    #[test]
    fn test_head_empty_body() {
        let req = Request::with_connector(
            Head, Url::parse("http://example.dom").unwrap(), &mut MockConnector
        ).unwrap();
        let bytes = run_request(req);
        let s = from_utf8(&bytes[..]).unwrap();
        assert_no_body(s);
    }

    #[test]
    fn test_url_query() {
        let url = Url::parse("http://example.dom?q=value").unwrap();
        let req = Request::with_connector(
            Get, url, &mut MockConnector
        ).unwrap();
        let bytes = run_request(req);
        let s = from_utf8(&bytes[..]).unwrap();
        assert!(s.contains("?q=value"));
    }

    #[test]
    fn test_post_content_length() {
        let url = Url::parse("http://example.dom").unwrap();
        let mut req = Request::with_connector(
            Post, url, &mut MockConnector
        ).unwrap();
        let mut body = String::new();
        form_urlencoded::Serializer::new(&mut body).append_pair("q", "value");
        req.headers_mut().set(ContentLength(body.len() as u64));
        let bytes = run_request(req);
        let s = from_utf8(&bytes[..]).unwrap();
        assert!(s.contains("Content-Length:"));
    }

    #[test]
    fn test_post_chunked() {
        let url = Url::parse("http://example.dom").unwrap();
        let req = Request::with_connector(
            Post, url, &mut MockConnector
        ).unwrap();
        let bytes = run_request(req);
        let s = from_utf8(&bytes[..]).unwrap();
        assert!(!s.contains("Content-Length:"));
    }

    #[test]
    fn test_host_header() {
        let url = Url::parse("http://example.dom").unwrap();
        let req = Request::with_connector(
            Get, url, &mut MockConnector
        ).unwrap();
        let bytes = run_request(req);
        let s = from_utf8(&bytes[..]).unwrap();
        assert!(s.contains("Host: example.dom"));
    }

    #[test]
    fn test_proxy() {
        let url = Url::parse("http://example.dom").unwrap();
        let mut req = Request::with_connector(
            Get, url, &mut MockConnector
        ).unwrap();
        req.message.set_proxied(true);
        let bytes = run_request(req);
        let s = from_utf8(&bytes[..]).unwrap();
        let request_line = "GET http://example.dom/ HTTP/1.1";
        assert_eq!(&s[..request_line.len()], request_line);
        assert!(s.contains("Host: example.dom"));
    }
    */
}
