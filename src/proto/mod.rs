//! Pieces pertaining to the HTTP message protocol.

/// The minimum value that can be set to max buffer size.
pub(crate) const MINIMUM_MAX_BUFFER_SIZE: usize = 8192;

/// The default maximum read buffer size. If the buffer gets this big and
/// a message is still not complete, a `TooLarge` error is triggered.
// Note: if this changes, update server::conn::Http::max_buf_size docs.
pub(crate) const DEFAULT_MAX_BUFFER_SIZE: usize = MINIMUM_MAX_BUFFER_SIZE + 4096 * 100;

cfg_feature! {
    #![feature = "http1"]

    pub(crate) mod h1;

    pub(crate) use self::h1::Conn;

    #[cfg(feature = "client")]
    pub(crate) use self::h1::dispatch;
    #[cfg(feature = "server")]
    pub(crate) use self::h1::ServerTransaction;
}

#[cfg(feature = "http2")]
pub(crate) mod h2;

/// An Incoming Message head. Includes request/status line, and headers.
#[derive(Debug, Default)]
pub(crate) struct MessageHead<S> {
    /// HTTP version of the message.
    pub(crate) version: http::Version,
    /// Subject (request line or status line) of Incoming message.
    pub(crate) subject: S,
    /// Headers of the Incoming message.
    pub(crate) headers: http::HeaderMap,
    /// Extensions.
    extensions: http::Extensions,
}

/// An incoming request message.
#[cfg(feature = "http1")]
pub(crate) type RequestHead = MessageHead<RequestLine>;

#[derive(Debug, Default, PartialEq)]
#[cfg(feature = "http1")]
pub(crate) struct RequestLine(pub(crate) http::Method, pub(crate) http::Uri);

/// An incoming response message.
#[cfg(all(feature = "http1", feature = "client"))]
pub(crate) type ResponseHead = MessageHead<http::StatusCode>;

#[derive(Debug)]
#[cfg(feature = "http1")]
pub(crate) enum BodyLength {
    /// Content-Length
    Known(u64),
    /// Transfer-Encoding: chunked (if h1)
    Unknown,
}

/// Status of when a Disaptcher future completes.
pub(crate) enum Dispatched {
    /// Dispatcher completely shutdown connection.
    Shutdown,
    /// Dispatcher has pending upgrade, and so did not shutdown.
    #[cfg(feature = "http1")]
    Upgrade(crate::upgrade::Pending),
}

impl MessageHead<http::StatusCode> {
    fn into_response<B>(self, body: B) -> http::Response<B> {
        let mut res = http::Response::new(body);
        *res.status_mut() = self.subject;
        *res.headers_mut() = self.headers;
        *res.version_mut() = self.version;
        *res.extensions_mut() = self.extensions;
        res
    }
}
