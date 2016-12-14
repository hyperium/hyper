//! Server Responses
//!
//! These are responses sent by a `hyper::Server` to clients, after
//! receiving a request.

use body::Body;
use header;
use http;
use status::StatusCode;
use version;

/// The outgoing half for a Tcp connection, created by a `Server` and given to a `Handler`.
///
/// The default `StatusCode` for a `Response` is `200 OK`.
#[derive(Default)]
pub struct Response {
    head: http::MessageHead<StatusCode>,
    body: Option<Body>,
}

impl Response {
    /// Create a new Response.
    #[inline]
    pub fn new() -> Response {
        Response::default()
    }

    /// The headers of this response.
    #[inline]
    pub fn headers(&self) -> &header::Headers { &self.head.headers }

    /// The status of this response.
    #[inline]
    pub fn status(&self) -> &StatusCode {
        &self.head.subject
    }

    /// The HTTP version of this response.
    #[inline]
    pub fn version(&self) -> &version::HttpVersion { &self.head.version }

    /// Get a mutable reference to the Headers.
    #[inline]
    pub fn headers_mut(&mut self) -> &mut header::Headers { &mut self.head.headers }

    /// Set the `StatusCode` for this response.
    #[inline]
    pub fn set_status(&mut self, status: StatusCode) {
        self.head.subject = status;
    }

    /// Set the body.
    #[inline]
    pub fn set_body<T: Into<Body>>(&mut self, body: T) {
        self.body = Some(body.into());
    }

    /// Set the status and move the Response.
    ///
    /// Useful for the "builder-style" pattern.
    #[inline]
    pub fn with_status(mut self, status: StatusCode) -> Self {
        self.set_status(status);
        self
    }

    /// Set a header and move the Response.
    ///
    /// Useful for the "builder-style" pattern.
    #[inline]
    pub fn with_header<H: header::Header>(mut self, header: H) -> Self {
        self.head.headers.set(header);
        self
    }

    /// Set the headers and move the Response.
    ///
    /// Useful for the "builder-style" pattern.
    #[inline]
    pub fn with_headers(mut self, headers: header::Headers) -> Self {
        self.head.headers = headers;
        self
    }

    /// Set the body and move the Response.
    ///
    /// Useful for the "builder-style" pattern.
    #[inline]
    pub fn with_body<T: Into<Body>>(mut self, body: T) -> Self {
        self.set_body(body);
        self
    }
}

pub fn split(res: Response) -> (http::MessageHead<StatusCode>, Option<Body>) {
    (res.head, res.body)
}
