//! Server Requests
//!
//! These are requests that a `hyper::Server` receives, and include its method,
//! target URI, headers, and message body.

use std::fmt;

use version::HttpVersion;
use method::Method;
use header::Headers;
use http::{RequestHead, MessageHead, RequestLine};
use uri::RequestUri;

pub fn new<'a, T>(incoming: RequestHead, transport: &'a T) -> Request<'a, T> {
    let MessageHead { version, subject: RequestLine(method, uri), headers } = incoming;
    debug!("Request Line: {:?} {:?} {:?}", method, uri, version);
    debug!("{:#?}", headers);

    Request {
        method: method,
        uri: uri,
        headers: headers,
        version: version,
        transport: transport,
    }
}

/// A request bundles several parts of an incoming `NetworkStream`, given to a `Handler`.
pub struct Request<'a, T: 'a> {
    method: Method,
    uri: RequestUri,
    version: HttpVersion,
    headers: Headers,
    transport: &'a T,
}

impl<'a, T> fmt::Debug for Request<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Request")
            .field("method", &self.method)
            .field("uri", &self.uri)
            .field("version", &self.version)
            .field("headers", &self.headers)
            .finish()
    }
}

impl<'a, T> Request<'a, T> {
    /// The `Method`, such as `Get`, `Post`, etc.
    #[inline]
    pub fn method(&self) -> &Method { &self.method }

    /// The headers of the incoming request.
    #[inline]
    pub fn headers(&self) -> &Headers { &self.headers }

    /// The underlying `Transport` of this request.
    #[inline]
    pub fn transport(&self) -> &'a T { self.transport }

    /// The target request-uri for this request.
    #[inline]
    pub fn uri(&self) -> &RequestUri { &self.uri }

    /// The version of HTTP for this request.
    #[inline]
    pub fn version(&self) -> &HttpVersion { &self.version }

    /*
    /// The target path of this Request.
    #[inline]
    pub fn path(&self) -> Option<&str> {
        match *self.uri {
            RequestUri::AbsolutePath(ref s) => Some(s),
            RequestUri::AbsoluteUri(ref url) => Some(&url[::url::Position::BeforePath..]),
            _ => None
        }
    }
    */

    /// Deconstruct this Request into its pieces.
    ///
    /// Modifying these pieces will have no effect on how hyper behaves.
    #[inline]
    pub fn deconstruct(self) -> (Method, RequestUri, HttpVersion, Headers) {
        (self.method, self.uri, self.version, self.headers)
    }

}
