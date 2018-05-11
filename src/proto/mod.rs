//! Pieces pertaining to the HTTP message protocol.
use http::{HeaderMap, Method, StatusCode, Uri, Version};

pub(crate) use self::h1::{dispatch, Conn, ClientTransaction, ClientUpgradeTransaction, ServerTransaction};

pub(crate) mod h1;
pub(crate) mod h2;


/// An Incoming Message head. Includes request/status line, and headers.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MessageHead<S> {
    /// HTTP version of the message.
    pub version: Version,
    /// Subject (request line or status line) of Incoming message.
    pub subject: S,
    /// Headers of the Incoming message.
    pub headers: HeaderMap,
}

/// An incoming request message.
pub type RequestHead = MessageHead<RequestLine>;

#[derive(Debug, Default, PartialEq)]
pub struct RequestLine(pub Method, pub Uri);

/// An incoming response message.
pub type ResponseHead = MessageHead<StatusCode>;

/*
impl<S> MessageHead<S> {
    pub fn should_keep_alive(&self) -> bool {
        should_keep_alive(self.version, &self.headers)
    }

    pub fn expecting_continue(&self) -> bool {
        expecting_continue(self.version, &self.headers)
    }
}

/// Checks if a connection should be kept alive.
#[inline]
pub fn should_keep_alive(version: Version, headers: &HeaderMap) -> bool {
    if version == Version::HTTP_10 {
        headers::connection_keep_alive(headers)
    } else {
        !headers::connection_close(headers)
    }
}

/// Checks if a connection is expecting a `100 Continue` before sending its body.
#[inline]
pub fn expecting_continue(version: Version, headers: &HeaderMap) -> bool {
    version == Version::HTTP_11 && headers::expect_continue(headers)
}
*/

#[derive(Debug)]
pub enum BodyLength {
    /// Content-Length
    Known(u64),
    /// Transfer-Encoding: chunked (if h1)
    Unknown,
}

/*
#[test]
fn test_should_keep_alive() {
    let mut headers = HeaderMap::new();

    assert!(!should_keep_alive(Version::HTTP_10, &headers));
    assert!(should_keep_alive(Version::HTTP_11, &headers));

    headers.insert("connection", ::http::header::HeaderValue::from_static("close"));
    assert!(!should_keep_alive(Version::HTTP_10, &headers));
    assert!(!should_keep_alive(Version::HTTP_11, &headers));

    headers.insert("connection", ::http::header::HeaderValue::from_static("keep-alive"));
    assert!(should_keep_alive(Version::HTTP_10, &headers));
    assert!(should_keep_alive(Version::HTTP_11, &headers));
}

#[test]
fn test_expecting_continue() {
    let mut headers = HeaderMap::new();

    assert!(!expecting_continue(Version::HTTP_10, &headers));
    assert!(!expecting_continue(Version::HTTP_11, &headers));

    headers.insert("expect", ::http::header::HeaderValue::from_static("100-continue"));
    assert!(!expecting_continue(Version::HTTP_10, &headers));
    assert!(expecting_continue(Version::HTTP_11, &headers));
}
*/
