//! Pieces pertaining to the HTTP message protocol.

cfg_http1! {
    pub(crate) mod h1;

    pub(crate) use self::h1::Conn;

    #[cfg(feature = "client")]
    pub(crate) use self::h1::dispatch;
    #[cfg(feature = "server")]
    pub(crate) use self::h1::ServerTransaction;
}

cfg_http2! {
    pub(crate) mod h2;
}

/// An Incoming Message head. Includes request/status line, and headers.
#[derive(Debug, Default)]
pub struct MessageHead<S> {
    /// HTTP version of the message.
    pub version: http::Version,
    /// Subject (request line or status line) of Incoming message.
    pub subject: S,
    /// Headers of the Incoming message.
    pub headers: http::HeaderMap,
    /// Extensions.
    extensions: http::Extensions,
}

/// An incoming request message.
#[cfg(feature = "http1")]
pub type RequestHead = MessageHead<RequestLine>;

#[derive(Debug, Default, PartialEq)]
#[cfg(feature = "http1")]
pub struct RequestLine(pub http::Method, pub http::Uri);

/// An incoming response message.
#[cfg(feature = "http1")]
#[cfg(feature = "client")]
pub type ResponseHead = MessageHead<http::StatusCode>;

#[derive(Debug)]
#[cfg(feature = "http1")]
pub enum BodyLength {
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
