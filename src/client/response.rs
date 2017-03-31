use std::fmt;

use header;
use http::{self, RawStatus, Body};
use status;
use version;

pub fn new(incoming: http::ResponseHead, body: Option<Body>) -> Response {
    trace!("Response::new");
    let status = status::StatusCode::from_u16(incoming.subject.0);
    debug!("version={:?}, status={:?}", incoming.version, status);
    debug!("headers={:?}", incoming.headers);

    Response {
        status: status,
        version: incoming.version,
        headers: incoming.headers,
        status_raw: incoming.subject,
        body: body,
    }

}

/// A response for a client request to a remote server.
pub struct Response {
    status: status::StatusCode,
    headers: header::Headers,
    version: version::HttpVersion,
    status_raw: RawStatus,
    body: Option<Body>,
}

impl Response {
    /// Get the headers from the server.
    #[inline]
    pub fn headers(&self) -> &header::Headers { &self.headers }

    /// Get the status from the server.
    #[inline]
    pub fn status(&self) -> status::StatusCode { self.status }

    /// Get the raw status code and reason.
    #[inline]
    pub fn status_raw(&self) -> &RawStatus { &self.status_raw }

    /// Get the HTTP version of this response from the server.
    #[inline]
    pub fn version(&self) -> version::HttpVersion { self.version }

    /// Take the `Body` of this response.
    #[inline]
    pub fn body(mut self) -> Body {
        self.body.take().unwrap_or(Body::empty())
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
