//! Pieces pertaining to the HTTP message protocol.
use std::borrow::Cow;
use std::fmt;

use bytes::BytesMut;

use header::{Connection, ConnectionOption};
use header::Headers;
use method::Method;
use status::StatusCode;
use uri::Uri;
use version::HttpVersion;
use version::HttpVersion::{Http10, Http11};

pub use self::conn::{Conn, KeepAlive, KA};
pub use self::body::{Body, TokioBody};
pub use self::chunk::Chunk;
pub use self::str::ByteStr;

mod body;
mod chunk;
mod conn;
mod io;
mod h1;
//mod h2;
mod str;

/*
macro_rules! nonblocking {
    ($e:expr) => ({
        match $e {
            Ok(n) => Ok(Some(n)),
            Err(e) => match e.kind() {
                stdio::ErrorKind::WouldBlock => Ok(None),
                _ => Err(e)
            }
        }
    });
}
*/

/// An Incoming Message head. Includes request/status line, and headers.
#[derive(Debug, Default, PartialEq)]
pub struct MessageHead<S> {
    /// HTTP version of the message.
    pub version: HttpVersion,
    /// Subject (request line or status line) of Incoming message.
    pub subject: S,
    /// Headers of the Incoming message.
    pub headers: Headers
}

/// An incoming request message.
pub type RequestHead = MessageHead<RequestLine>;

#[derive(Debug, Default, PartialEq)]
pub struct RequestLine(pub Method, pub Uri);

impl fmt::Display for RequestLine {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {}", self.0, self.1)
    }
}

/// An incoming response message.
pub type ResponseHead = MessageHead<RawStatus>;

impl<S> MessageHead<S> {
    pub fn should_keep_alive(&self) -> bool {
        should_keep_alive(self.version, &self.headers)
    }
}

impl ResponseHead {
    /// Converts this head's RawStatus into a StatusCode.
    #[inline]
    pub fn status(&self) -> StatusCode {
        self.subject.status()
    }
}

/// The raw status code and reason-phrase.
#[derive(Clone, PartialEq, Debug)]
pub struct RawStatus(pub u16, pub Cow<'static, str>);

impl RawStatus {
    /// Converts this into a StatusCode.
    #[inline]
    pub fn status(&self) -> StatusCode {
        StatusCode::from_u16(self.0)
    }
}

impl fmt::Display for RawStatus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {}", self.0, self.1)
    }
}

impl From<StatusCode> for RawStatus {
    fn from(status: StatusCode) -> RawStatus {
        RawStatus(status.into(), Cow::Borrowed(status.canonical_reason().unwrap_or("")))
    }
}

impl Default for RawStatus {
    fn default() -> RawStatus {
        RawStatus(200, Cow::Borrowed("OK"))
    }
}

impl From<MessageHead<::StatusCode>> for MessageHead<RawStatus> {
    fn from(head: MessageHead<::StatusCode>) -> MessageHead<RawStatus> {
        MessageHead {
            subject: head.subject.into(),
            version: head.version,
            headers: head.headers,
        }
    }
}

/// Checks if a connection should be kept alive.
#[inline]
pub fn should_keep_alive(version: HttpVersion, headers: &Headers) -> bool {
    let ret = match (version, headers.get::<Connection>()) {
        (Http10, None) => false,
        (Http10, Some(conn)) if !conn.contains(&ConnectionOption::KeepAlive) => false,
        (Http11, Some(conn)) if conn.contains(&ConnectionOption::Close)  => false,
        _ => true
    };
    trace!("should_keep_alive(version={:?}, header={:?}) = {:?}", version, headers.get::<Connection>(), ret);
    ret
}

#[derive(Debug)]
pub enum ServerTransaction {}

#[derive(Debug)]
pub enum ClientTransaction {}

pub trait Http1Transaction {
    type Incoming;
    type Outgoing: Default;
    fn parse(bytes: &mut BytesMut) -> ParseResult<Self::Incoming>;
    fn decoder(head: &MessageHead<Self::Incoming>) -> ::Result<h1::Decoder>;
    fn encode(head: &mut MessageHead<Self::Outgoing>, dst: &mut Vec<u8>) -> h1::Encoder;
    fn should_set_length(head: &MessageHead<Self::Outgoing>) -> bool;
}

pub type ParseResult<T> = ::Result<Option<(MessageHead<T>, usize)>>;

struct DebugTruncate<'a>(&'a [u8]);

impl<'a> fmt::Debug for DebugTruncate<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let bytes = self.0;
        if bytes.len() > 32 {
            try!(f.write_str("["));
            for byte in &bytes[..32] {
                try!(write!(f, "{:?}, ", byte));
            }
            write!(f, "... {}]", bytes.len())
        } else {
            fmt::Debug::fmt(bytes, f)
        }
    }
}

#[test]
fn test_should_keep_alive() {
    let mut headers = Headers::new();

    assert!(!should_keep_alive(Http10, &headers));
    assert!(should_keep_alive(Http11, &headers));

    headers.set(Connection::close());
    assert!(!should_keep_alive(Http10, &headers));
    assert!(!should_keep_alive(Http11, &headers));

    headers.set(Connection::keep_alive());
    assert!(should_keep_alive(Http10, &headers));
    assert!(should_keep_alive(Http11, &headers));
}
