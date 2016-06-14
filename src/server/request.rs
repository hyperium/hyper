//! Server Requests
//!
//! These are requests that a `hyper::Server` receives, and include its method,
//! target URI, headers, and message body.

use version::HttpVersion;
use method::Method;
use header::Headers;
use http::{RequestHead, MessageHead, RequestLine};
use uri::RequestUri;

pub fn new(incoming: RequestHead) -> Request {
    let MessageHead { version, subject: RequestLine(method, uri), headers } = incoming;
    debug!("Request Line: {:?} {:?} {:?}", method, uri, version);
    debug!("{:#?}", headers);

    Request {
        method: method,
        uri: uri,
        headers: headers,
        version: version,
    }
}

/// A request bundles several parts of an incoming `NetworkStream`, given to a `Handler`.
#[derive(Debug)]
pub struct Request {
    // The IP address of the remote connection.
    //remote_addr: SocketAddr,
    method: Method,
    headers: Headers,
    uri: RequestUri,
    version: HttpVersion,
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
