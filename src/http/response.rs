use std::fmt;

use header::{Header, Headers};
use http::{MessageHead, ResponseHead, Body, RawStatus};
use status::StatusCode;
use version::HttpVersion;

/// A response for a client request to a remote server.
#[derive(Clone)]
pub struct Response<B = Option<Body>> {
    version: HttpVersion,
    headers: Headers,
    status: StatusCode,
    raw_status: RawStatus,
    body: B,
}

impl Response {
    /// Constructs a default response
    #[inline]
    pub fn new() -> Response {
        Response::default()
    }
}

impl<B> Response<B> {
    /// Constructs a default response with a given body
    #[inline]
    pub fn new_with_body(body: B) -> Response<B> {
        Response::<()>::default().with_body(body)
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
    pub fn status_raw(&self) -> &RawStatus { &self.raw_status }

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
        self.body = body.into();
    }

    /// Set the body and move the Response.
    ///
    /// Useful for the "builder-style" pattern.
    #[inline]
    pub fn with_body<B_>(self, body: B_) -> Response<B_> {
        let Response {
            version: version_,
            headers: headers_,
            status: status_,
            raw_status: raw_status_,
            ..
        } = self;

        Response {
            version: version_,
            headers: headers_,
            status: status_,
            raw_status: raw_status_,
            body: body,
        }
    }

    /// Get a reference to the body.
    pub fn body(&self) -> &B { &self.body }

    /// Get a mutable reference to the body.
    pub fn body_mut(&mut self) -> &mut B { &mut self.body }

    /// Take the body, moving it out of the Response.
    #[inline]
    pub fn take_body(self) -> (B, Response<()>) {
        let Response {
            version: version_,
            headers: headers_,
            status: status_,
            raw_status: raw_status_,
            body: body_,
        } = self;

        (body_, Response {
            version: version_,
            headers: headers_,
            status: status_,
            raw_status: raw_status_,
            body: (),
        })
    }

    /// Take the body of this response.
    #[inline]
    pub fn into_body(self) -> B {
        self.body
    }
}

impl<B: Default> Default for Response<B> {
    fn default() -> Response<B> {
        Response::<B> {
            version: Default::default(),
            headers: Default::default(),
            status: Default::default(),
            raw_status: Default::default(),
            body: Default::default(),
        }
    }
}

impl<B> fmt::Debug for Response<B> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Response")
            .field("status", &self.status)
            .field("version", &self.version)
            .field("headers", &self.headers)
            .finish()
    }
}

/// Constructs a response using a received ResponseHead and optional body
#[inline]
pub fn from_wire<B: Default>(incoming: ResponseHead, body: Option<B>)
                             -> Response<Option<B>> {
    let status = incoming.status();
    trace!("Response::new");
    debug!("version={:?}, status={:?}", incoming.version, status);
    debug!("headers={:?}", incoming.headers);

    Response {
        status: status,
        version: incoming.version,
        headers: incoming.headers,
        raw_status: incoming.subject,
        body: body,
    }
}

/// Splits this response into a MessageHead<StatusCode> and its body
#[inline]
pub fn split<B>(res: Response<B>) -> (MessageHead<StatusCode>, B) {
    let head = MessageHead::<StatusCode> {
        version: res.version,
        headers: res.headers,
        subject: res.status
    };
    (head, res.body)
}
