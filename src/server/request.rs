//! Server Requests
//!
//! These are requests that a `hyper::Server` receives, and include its method,
//! target URI, headers, and message body.

use std::fmt;
use std::net::SocketAddr;

use version::HttpVersion;
use method::Method;
use header::Headers;
use http::{RequestHead, MessageHead, RequestLine, Body};
use uri::Uri;

/// A request bundles several parts of an incoming `NetworkStream`, given to a `Handler`.
pub struct Request {
    method: Method,
    uri: Uri,
    version: HttpVersion,
    headers: Headers,
    remote_addr: Option<SocketAddr>,
    body: Body,
}

impl Request {
    /// The `Method`, such as `Get`, `Post`, etc.
    #[inline]
    pub fn method(&self) -> &Method { &self.method }

    /// The headers of the incoming request.
    #[inline]
    pub fn headers(&self) -> &Headers { &self.headers }

    /// The target request-uri for this request.
    #[inline]
    pub fn uri(&self) -> &Uri { &self.uri }

    /// The version of HTTP for this request.
    #[inline]
    pub fn version(&self) -> HttpVersion { self.version }

    /// The remote socket address of this request
    ///
    /// This is an `Option`, because some underlying transports may not have
    /// a socket address, such as Unix Sockets.
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

    /// Take the `Body` of this `Request`.
    #[inline]
    pub fn body(self) -> Body {
        self.body
    }

    /// Deconstruct this Request into its pieces.
    ///
    /// Modifying these pieces will have no effect on how hyper behaves.
    #[inline]
    pub fn deconstruct(self) -> (Method, Uri, HttpVersion, Headers, Body) {
        (self.method, self.uri, self.version, self.headers, self.body)
    }
}

impl fmt::Debug for Request {
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

pub fn new(addr: Option<SocketAddr>, incoming: RequestHead, body: Body) -> Request {
    let MessageHead { version, subject: RequestLine(method, uri), headers } = incoming;
    debug!("Request::new: addr={}, req=\"{} {} {}\"", MaybeAddr(&addr), method, uri, version);
    debug!("Request::new: headers={:?}", headers);

    Request {
        method: method,
        uri: uri,
        headers: headers,
        version: version,
        remote_addr: addr,
        body: body,
    }
}

struct MaybeAddr<'a>(&'a Option<SocketAddr>);

impl<'a> fmt::Display for MaybeAddr<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self.0 {
            Some(ref addr) => fmt::Display::fmt(addr, f),
            None => f.write_str("None"),
        }
    }
}

