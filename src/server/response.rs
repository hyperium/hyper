//! Server Responses
//!
//! These are responses sent by a `hyper::Server` to clients, after
//! receiving a request.

use futures::Future;

use header;
use http;
use status::StatusCode;
use version;

type Body = ::tokio_proto::Body<http::Chunk, ::Error>;

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

    pub fn status(mut self, status: StatusCode) -> Self {
        self.head.subject = status;
        self
    }

    pub fn header<H: header::Header>(mut self, header: H) -> Self {
        self.head.headers.set(header);
        self
    }

    pub fn headers(mut self, headers: header::Headers) -> Self {
        self.head.headers = headers;
        self
    }

    //pub fn body(mut self, buf: &'static [u8]) -> Self {
    pub fn body<T: IntoBody>(mut self, body: T) -> Self {
        self.body = Some(body.into());
        self
    }

    /*
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

    /// Get a mutable reference to the status.
    #[inline]
    pub fn set_status(&mut self, status: StatusCode) {
        self.head.subject = status;
    }
    */
}

pub fn split(res: Response) -> (http::MessageHead<StatusCode>, Option<Body>) {
    (res.head, res.body)
}

pub trait IntoBody {
    fn into(self) -> Body;
}

impl IntoBody for Body {
    fn into(self) -> Self {
        self
    }
}

impl IntoBody for Vec<u8> {
    fn into(self) -> Body {
        let (tx, rx) = Body::pair();
        tx.send(Ok(http::Chunk::from(self))).poll();
        rx
    }
}

impl IntoBody for &'static [u8] {
    fn into(self) -> Body {
        let vec = self.to_vec();
        IntoBody::into(vec)
    }
}
