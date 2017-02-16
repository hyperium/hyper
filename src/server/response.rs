use std::fmt;

use header;
use http::{self, Body};
use status::StatusCode;
use version;

/// The Response sent to a client after receiving a Request in a Service.
///
/// The default `StatusCode` for a `Response` is `200 OK`.
pub struct Response<B = Body> {
    head: http::MessageHead<StatusCode>,
    body: Option<B>,
}

impl<B> Response<B> {
    /// Create a new Response.
    #[inline]
    pub fn new() -> Response<B> {
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
    pub fn set_body<T: Into<B>>(&mut self, body: T) {
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
    pub fn with_body<T: Into<B>>(mut self, body: T) -> Self {
        self.set_body(body);
        self
    }
}

impl<B> Default for Response<B> {
    fn default() -> Response<B> {
        Response {
            head: Default::default(),
            body: None,
        }
    }
}

impl<B> fmt::Debug for Response<B> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Response")
            .field("status", &self.head.subject)
            .field("version", &self.head.version)
            .field("headers", &self.head.headers)
            .finish()
    }
}

pub fn split<B>(res: Response<B>) -> (http::MessageHead<StatusCode>, Option<B>) {
    (res.head, res.body)
}
