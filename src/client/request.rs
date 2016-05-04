//! Client Requests
use std::marker::PhantomData;
use std::io::{self, Write};

use std::time::Duration;

use url::Url;

use method::Method;
use header::Headers;
use header::Host;
use net::{NetworkStream, NetworkConnector, DefaultConnector, Fresh, Streaming};
use version;
use client::{Response, get_host_and_port};

use http::{HttpMessage, RequestHead};
use http::h1::Http11Message;


/// A client request to a remote server.
/// The W type tracks the state of the request, Fresh vs Streaming.
pub struct Request<W> {
    /// The target URI for this request.
    pub url: Url,

    /// The HTTP version of this request.
    pub version: version::HttpVersion,

    message: Box<HttpMessage>,
    headers: Headers,
    method: Method,

    _marker: PhantomData<W>,
}

impl<W> Request<W> {
    /// Read the Request headers.
    #[inline]
    pub fn headers(&self) -> &Headers { &self.headers }

    /// Read the Request method.
    #[inline]
    pub fn method(&self) -> Method { self.method.clone() }

    /// Set the write timeout.
    #[inline]
    pub fn set_write_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        self.message.set_write_timeout(dur)
    }

    /// Set the read timeout.
    #[inline]
    pub fn set_read_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        self.message.set_read_timeout(dur)
    }
}

impl Request<Fresh> {
    /// Create a new `Request<Fresh>` that will use the given `HttpMessage` for its communication
    /// with the server. This implies that the given `HttpMessage` instance has already been
    /// properly initialized by the caller (e.g. a TCP connection's already established).
    pub fn with_message(method: Method, url: Url, message: Box<HttpMessage>)
            -> ::Result<Request<Fresh>> {
        let mut headers = Headers::new();
        {
            let (host, port) = try!(get_host_and_port(&url));
            headers.set(Host {
                hostname: host.to_owned(),
                port: Some(port),
            });
        }

        Ok(Request::with_headers_and_message(method, url, headers, message))
    }

    #[doc(hidden)]
    pub fn with_headers_and_message(method: Method, url: Url, headers: Headers,  message: Box<HttpMessage>)
                -> Request<Fresh> {
        Request {
            method: method,
            headers: headers,
            url: url,
            version: version::HttpVersion::Http11,
            message: message,
            _marker: PhantomData,
        }
    }

    /// Create a new client request.
    pub fn new(method: Method, url: Url) -> ::Result<Request<Fresh>> {
        let conn = DefaultConnector::default();
        Request::with_connector(method, url, &conn)
    }

    /// Create a new client request with a specific underlying NetworkStream.
    pub fn with_connector<C, S>(method: Method, url: Url, connector: &C)
        -> ::Result<Request<Fresh>> where
        C: NetworkConnector<Stream=S>,
        S: Into<Box<NetworkStream + Send>> {
        let stream = {
            let (host, port) = try!(get_host_and_port(&url));
            try!(connector.connect(host, port, url.scheme())).into()
        };

        Request::with_message(method, url, Box::new(Http11Message::with_stream(stream)))
    }

    /// Consume a Fresh Request, writing the headers and method,
    /// returning a Streaming Request.
    pub fn start(mut self) -> ::Result<Request<Streaming>> {
        let head = match self.message.set_outgoing(RequestHead {
            headers: self.headers,
            method: self.method,
            url: self.url,
        }) {
            Ok(head) => head,
            Err(e) => {
                let _ = self.message.close_connection();
                return Err(From::from(e));
            }
        };

        Ok(Request {
            method: head.method,
            headers: head.headers,
            url: head.url,
            version: self.version,
            message: self.message,
            _marker: PhantomData,
        })
    }

    /// Get a mutable reference to the Request headers.
    #[inline]
    pub fn headers_mut(&mut self) -> &mut Headers { &mut self.headers }
}



impl Request<Streaming> {
    /// Completes writing the request, and returns a response to read from.
    ///
    /// Consumes the Request.
    pub fn send(self) -> ::Result<Response> {
        Response::with_message(self.url, self.message)
    }
}

impl Write for Request<Streaming> {
    #[inline]
    fn write(&mut self, msg: &[u8]) -> io::Result<usize> {
        match self.message.write(msg) {
            Ok(n) => Ok(n),
            Err(e) => {
                let _ = self.message.close_connection();
                Err(e)
            }
        }
    }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        match self.message.flush() {
            Ok(r) => Ok(r),
            Err(e) => {
                let _ = self.message.close_connection();
                Err(e)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::str::from_utf8;
    use url::Url;
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

    #[test]
    fn test_post_chunked_with_encoding() {
        let url = Url::parse("http://example.dom").unwrap();
        let mut req = Request::with_connector(
            Post, url, &mut MockConnector
        ).unwrap();
        req.headers_mut().set(TransferEncoding(vec![Encoding::Chunked]));
        let bytes = run_request(req);
        let s = from_utf8(&bytes[..]).unwrap();
        assert!(!s.contains("Content-Length:"));
        assert!(s.contains("Transfer-Encoding:"));
    }

    #[test]
    fn test_write_error_closes() {
        let url = Url::parse("http://hyper.rs").unwrap();
        let req = Request::with_connector(
            Get, url, &mut MockConnector
        ).unwrap();
        let mut req = req.start().unwrap();

        req.message.downcast_mut::<Http11Message>().unwrap()
            .get_mut().downcast_mut::<MockStream>().unwrap()
            .error_on_write = true;

        req.write(b"foo").unwrap();
        assert!(req.flush().is_err());

        assert!(req.message.downcast_ref::<Http11Message>().unwrap()
            .get_ref().downcast_ref::<MockStream>().unwrap()
            .is_closed);
    }
}
