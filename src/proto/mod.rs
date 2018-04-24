//! Pieces pertaining to the HTTP message protocol.
use bytes::BytesMut;
use http::{HeaderMap, Method, StatusCode, Uri, Version};

use headers;

pub(crate) use self::h1::{dispatch, Conn};

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

pub(crate) type ServerTransaction = h1::role::Server<h1::role::YesUpgrades>;
//pub type ServerTransaction = h1::role::Server<h1::role::NoUpgrades>;
//pub type ServerUpgradeTransaction = h1::role::Server<h1::role::YesUpgrades>;

pub(crate) type ClientTransaction = h1::role::Client<h1::role::NoUpgrades>;
pub(crate) type ClientUpgradeTransaction = h1::role::Client<h1::role::YesUpgrades>;

pub(crate) trait Http1Transaction {
    type Incoming;
    type Outgoing: Default;
    fn parse(bytes: &mut BytesMut) -> ParseResult<Self::Incoming>;
    fn decoder(head: &MessageHead<Self::Incoming>, method: &mut Option<Method>) -> ::Result<Decode>;
    fn encode(
        head: MessageHead<Self::Outgoing>,
        body: Option<BodyLength>,
        method: &mut Option<Method>,
        title_case_headers: bool,
        dst: &mut Vec<u8>,
    ) -> ::Result<h1::Encoder>;
    fn on_error(err: &::Error) -> Option<MessageHead<Self::Outgoing>>;

    fn should_error_on_parse_eof() -> bool;
    fn should_read_first() -> bool;
}

pub(crate) type ParseResult<T> = Result<Option<(MessageHead<T>, usize)>, ::error::Parse>;

#[derive(Debug)]
pub enum BodyLength {
    /// Content-Length
    Known(u64),
    /// Transfer-Encoding: chunked (if h1)
    Unknown,
}


#[derive(Debug)]
pub enum Decode {
    /// Decode normally.
    Normal(h1::Decoder),
    /// After this decoder is done, HTTP is done.
    Final(h1::Decoder),
    /// A header block that should be ignored, like unknown 1xx responses.
    Ignore,
}

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
