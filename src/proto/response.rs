use std::fmt;
#[cfg(feature = "compat")]
use std::mem::replace;

#[cfg(feature = "compat")]
use http;

use header::{Header, Headers};
use proto::{MessageHead, ResponseHead, Body};
use status::StatusCode;
use version::HttpVersion;

/// An HTTP Response
pub struct Response<B = Body> {
    version: HttpVersion,
    headers: Headers,
    status: StatusCode,
    #[cfg(feature = "raw_status")]
    raw_status: ::proto::RawStatus,
    body: Option<B>,
}

impl<B> Response<B> {
    /// Constructs a default response
    #[inline]
    pub fn new() -> Response<B> {
        Response::default()
    }

    /// Get the HTTP version of this response.
    #[inline]
    pub fn version(&self) -> HttpVersion { self.version }

    /// Get the headers from the response.
    #[inline]
    pub fn headers(&self) -> &Headers { &self.headers }

    /// Get a mutable reference to the headers.
    #[inline]
    pub fn headers_mut(&mut self) -> &mut Headers { &mut self.headers }

    /// Get the status from the server.
    #[inline]
    pub fn status(&self) -> StatusCode { self.status }

    /// Get the raw status code and reason.
    ///
    /// This method is only useful when inspecting the raw subject line from
    /// a received response.
    #[inline]
    #[cfg(feature = "raw_status")]
    pub fn status_raw(&self) -> &::proto::RawStatus { &self.raw_status }

    /// Set the `StatusCode` for this response.
    #[inline]
    pub fn set_status(&mut self, status: StatusCode) {
        self.status = status;
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
    pub fn with_header<H: Header>(mut self, header: H) -> Self {
        self.headers.set(header);
        self
    }

    /// Set the headers and move the Response.
    ///
    /// Useful for the "builder-style" pattern.
    #[inline]
    pub fn with_headers(mut self, headers: Headers) -> Self {
        self.headers = headers;
        self
    }

    /// Set the body.
    #[inline]
    pub fn set_body<T: Into<B>>(&mut self, body: T) {
        self.body = Some(body.into());
    }

    /// Set the body and move the Response.
    ///
    /// Useful for the "builder-style" pattern.
    #[inline]
    pub fn with_body<T: Into<B>>(mut self, body: T) -> Self {
        self.set_body(body);
        self
    }

    /// Read the body.
    #[inline]
    pub fn body_ref(&self) -> Option<&B> { self.body.as_ref() }
}

impl Response<Body> {
    /// Take the `Body` of this response.
    #[inline]
    pub fn body(self) -> Body {
        self.body.unwrap_or_default()
    }
}

#[cfg(not(feature = "raw_status"))]
impl<B> Default for Response<B> {
    fn default() -> Response<B> {
        Response::<B> {
            version: Default::default(),
            headers: Default::default(),
            status: Default::default(),
            body: None,
        }
    }
}

#[cfg(feature = "raw_status")]
impl<B> Default for Response<B> {
    fn default() -> Response<B> {
        Response::<B> {
            version: Default::default(),
            headers: Default::default(),
            status: Default::default(),
            raw_status: Default::default(),
            body: None,
        }
    }
}

impl fmt::Debug for Response {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Response")
            .field("status", &self.status)
            .field("version", &self.version)
            .field("headers", &self.headers)
            .finish()
    }
}

#[cfg(feature = "compat")]
impl<B> From<http::Response<B>> for Response<B> {
    fn from(from_res: http::Response<B>) -> Response<B> {
        let (from_parts, body) = from_res.into_parts();
        let mut to_res = Response::new();
        to_res.version = from_parts.version.into();
        to_res.set_status(from_parts.status.into());
        replace(to_res.headers_mut(), from_parts.headers.into());
        to_res.with_body(body)
    }
}

#[cfg(feature = "compat")]
impl From<Response> for http::Response<Body> {
    fn from(mut from_res: Response) -> http::Response<Body> {
        let (mut to_parts, ()) = http::Response::new(()).into_parts();
        to_parts.version = from_res.version().into();
        to_parts.status = from_res.status().into();
        let from_headers = replace(from_res.headers_mut(), Headers::new());
        to_parts.headers = from_headers.into();
        http::Response::from_parts(to_parts, from_res.body())
    }
}

/// Constructs a response using a received ResponseHead and optional body
#[inline]
#[cfg(not(feature = "raw_status"))]
pub fn from_wire<B>(incoming: ResponseHead, body: Option<B>) -> Response<B> {
    let status = incoming.status();

    Response::<B> {
        status: status,
        version: incoming.version,
        headers: incoming.headers,
        body: body,
    }
}

/// Constructs a response using a received ResponseHead and optional body
#[inline]
#[cfg(feature = "raw_status")]
pub fn from_wire<B>(incoming: ResponseHead, body: Option<B>) -> Response<B> {
    let status = incoming.status();

    Response::<B> {
        status: status,
        version: incoming.version,
        headers: incoming.headers,
        raw_status: incoming.subject,
        body: body,
    }
}

/// Splits this response into a `MessageHead<StatusCode>` and its body
#[inline]
pub fn split<B>(res: Response<B>) -> (MessageHead<StatusCode>, Option<B>) {
    let head = MessageHead::<StatusCode> {
        version: res.version,
        headers: res.headers,
        subject: res.status
    };
    (head, res.body)
}
