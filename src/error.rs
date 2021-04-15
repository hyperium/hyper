//! Error and Result module.
use std::error::Error as StdError;
use std::fmt;

use self::sealed::Sealed;

/// Result type often returned from methods that can have hyper `Error`s.
pub type Result<T> = std::result::Result<T, Error>;

type Cause = Box<dyn StdError + Send + Sync>;

/// Represents errors that can occur handling HTTP streams.
pub struct Error {
    inner: Box<ErrorImpl>,
}

struct ErrorImpl {
    kind: Kind,
    cause: Option<Cause>,
}

/// Represents the kind of the error.
///
/// This enum is non-exhaustive.
#[non_exhaustive]
pub enum Kind {
    /// Error occured while parsing.
    Parse(Parse),
    /// Error occured while executing user code.
    User(User),
    /// A message reached EOF, but is not complete.
    IncompleteMessage(Sealed),
    /// A connection received a message (or bytes) when not waiting for one.
    #[cfg(feature = "http1")]
    UnexpectedMessage(Sealed),
    /// A pending item was dropped before ever being processed.
    Canceled(Sealed),
    /// Indicates a channel (client or body sender) is closed.
    ChannelClosed(Sealed),
    /// An `io::Error` that occurred while trying to read or write to a network stream.
    #[cfg(any(feature = "http1", feature = "http2"))]
    Io(Sealed),
    /// Error occurred while connecting.
    Connect(Sealed),
    /// Error creating a TcpListener.
    #[cfg(all(
        any(feature = "http1", feature = "http2"),
        feature = "tcp",
        feature = "server"
    ))]
    Listen(Sealed),
    /// Error accepting on an Incoming stream.
    #[cfg(any(feature = "http1", feature = "http2"))]
    #[cfg(feature = "server")]
    Accept(Sealed),
    /// Error while reading a body from connection.
    #[cfg(any(feature = "http1", feature = "http2", feature = "stream"))]
    Body(Sealed),
    /// Error while writing a body to connection.
    #[cfg(any(feature = "http1", feature = "http2"))]
    BodyWrite(Sealed),
    /// The body write was aborted.
    BodyWriteAborted(Sealed),
    /// Error calling AsyncWrite::shutdown()
    #[cfg(feature = "http1")]
    Shutdown(Sealed),

    /// A general error from h2.
    #[cfg(feature = "http2")]
    Http2(Sealed),
}

/// Represents the kind of the parse error.
///
/// This enum is non-exhaustive.
#[non_exhaustive]
pub enum Parse {
    /// Invalid HTTP method parsed.
    Method(Sealed),
    /// Invalid HTTP version parsed.
    Version(Sealed),
    /// Found HTTP/2 preface.
    #[cfg(feature = "http1")]
    H2Preface(Sealed),
    /// Invalid URI.
    Uri(Sealed),
    /// Invalid HTTP header parsed.
    Header(Sealed),
    /// Header section is too large.
    HeaderSectionTooLarge(Sealed),
    /// Invalid HTTP status-code parsed.
    Status(Sealed),
}

/// Represents the kind of the user error.
///
/// This enum is non-exhaustive.
#[non_exhaustive]
pub enum User {
    /// Error calling user's HttpBody::poll_data().
    #[cfg(any(feature = "http1", feature = "http2"))]
    Body(Sealed),
    /// Error calling user's MakeService.
    #[cfg(any(feature = "http1", feature = "http2"))]
    #[cfg(feature = "server")]
    MakeService(Sealed),
    /// Error from future of user's Service.
    #[cfg(any(feature = "http1", feature = "http2"))]
    Service(Sealed),
    /// User tried to send a certain header in an unexpected context.
    ///
    /// For example, sending both `content-length` and `transfer-encoding`.
    #[cfg(feature = "http1")]
    #[cfg(feature = "server")]
    UnexpectedHeader(Sealed),
    /// User tried to create a Request with bad version.
    #[cfg(any(feature = "http1", feature = "http2"))]
    #[cfg(feature = "client")]
    UnsupportedVersion(Sealed),
    /// User tried to create a CONNECT Request with the Client.
    #[cfg(any(feature = "http1", feature = "http2"))]
    #[cfg(feature = "client")]
    UnsupportedRequestMethod(Sealed),
    /// User tried to respond with a 1xx (not 101) response code.
    #[cfg(feature = "http1")]
    #[cfg(feature = "server")]
    UnsupportedStatusCode(Sealed),
    /// User tried to send a Request with Client with non-absolute URI.
    #[cfg(any(feature = "http1", feature = "http2"))]
    #[cfg(feature = "client")]
    AbsoluteUriRequired(Sealed),

    /// User tried polling for an upgrade that doesn't exist.
    NoUpgrade(Sealed),

    /// User polled for an upgrade, but low-level API is not using upgrades.
    #[cfg(feature = "http1")]
    ManualUpgrade(Sealed),

    /// User aborted in an FFI callback.
    #[cfg(feature = "ffi")]
    AbortedByCallback(Sealed),
}

// Sentinel type to indicate the error was caused by a timeout.
#[derive(Debug)]
pub(super) struct TimedOut;

impl Error {
    /// Returns true if this was an HTTP parse error.
    pub fn is_parse(&self) -> bool {
        matches!(self.inner.kind, Kind::Parse(_))
    }

    /// Returns true if this error was caused by user code.
    pub fn is_user(&self) -> bool {
        matches!(self.inner.kind, Kind::User(_))
    }

    /// Returns true if this was about a `Request` that was canceled.
    pub fn is_canceled(&self) -> bool {
        matches!(self.inner.kind, Kind::Canceled(_))
    }

    /// Returns true if a sender's channel is closed.
    pub fn is_closed(&self) -> bool {
        matches!(self.inner.kind, Kind::ChannelClosed(_))
    }

    /// Returns true if this was an error from `Connect`.
    pub fn is_connect(&self) -> bool {
        matches!(self.inner.kind, Kind::Connect(_))
    }

    /// Returns true if the connection closed before a message could complete.
    pub fn is_incomplete_message(&self) -> bool {
        matches!(self.inner.kind, Kind::IncompleteMessage(_))
    }

    /// Returns true if the body write was aborted.
    pub fn is_body_write_aborted(&self) -> bool {
        matches!(self.inner.kind, Kind::BodyWriteAborted(_))
    }

    /// Returns true if the error was caused by a timeout.
    pub fn is_timeout(&self) -> bool {
        self.find_source::<TimedOut>().is_some()
    }

    /// Consumes the error, returning its cause.
    pub fn into_cause(self) -> Option<Box<dyn StdError + Send + Sync>> {
        self.inner.cause
    }

    pub(super) fn new(kind: Kind) -> Error {
        Error {
            inner: Box::new(ErrorImpl { kind, cause: None }),
        }
    }

    pub(super) fn with<C: Into<Cause>>(mut self, cause: C) -> Error {
        self.inner.cause = Some(cause.into());
        self
    }

    /// Returns the kind of the error.
    #[cfg(any(all(feature = "http1", feature = "server"), feature = "ffi"))]
    pub fn kind(&self) -> &Kind {
        &self.inner.kind
    }

    fn find_source<E: StdError + 'static>(&self) -> Option<&E> {
        let mut cause = self.source();
        while let Some(err) = cause {
            if let Some(ref typed) = err.downcast_ref() {
                return Some(typed);
            }
            cause = err.source();
        }

        // else
        None
    }

    #[cfg(feature = "http2")]
    pub(super) fn h2_reason(&self) -> h2::Reason {
        // Find an h2::Reason somewhere in the cause stack, if it exists,
        // otherwise assume an INTERNAL_ERROR.
        self.find_source::<h2::Error>()
            .and_then(|h2_err| h2_err.reason())
            .unwrap_or(h2::Reason::INTERNAL_ERROR)
    }

    pub(super) fn new_parse(err: Parse) -> Error {
        Error::new(Kind::Parse(err))
    }

    pub(super) fn new_canceled() -> Error {
        Error::new(Kind::Canceled(Sealed))
    }

    #[cfg(feature = "http1")]
    pub(super) fn new_incomplete() -> Error {
        Error::new(Kind::IncompleteMessage(Sealed))
    }

    #[cfg(feature = "http1")]
    pub(super) fn new_too_large() -> Error {
        Error::new(Kind::Parse(Parse::new_header_section_too_large()))
    }

    #[cfg(feature = "http1")]
    pub(super) fn new_h2_preface() -> Error {
        Error::new(Kind::Parse(Parse::new_h2_preface()))
    }

    #[cfg(feature = "http1")]
    pub(super) fn new_unexpected_message() -> Error {
        Error::new(Kind::UnexpectedMessage(Sealed))
    }

    #[cfg(any(feature = "http1", feature = "http2"))]
    pub(super) fn new_io(cause: std::io::Error) -> Error {
        Error::new(Kind::Io(Sealed)).with(cause)
    }

    #[cfg(all(any(feature = "http1", feature = "http2"), feature = "tcp"))]
    #[cfg(feature = "server")]
    pub(super) fn new_listen<E: Into<Cause>>(cause: E) -> Error {
        Error::new(Kind::Listen(Sealed)).with(cause)
    }

    #[cfg(any(feature = "http1", feature = "http2"))]
    #[cfg(feature = "server")]
    pub(super) fn new_accept<E: Into<Cause>>(cause: E) -> Error {
        Error::new(Kind::Accept(Sealed)).with(cause)
    }

    #[cfg(any(feature = "http1", feature = "http2"))]
    #[cfg(feature = "client")]
    pub(super) fn new_connect<E: Into<Cause>>(cause: E) -> Error {
        Error::new(Kind::Connect(Sealed)).with(cause)
    }

    pub(super) fn new_closed() -> Error {
        Error::new(Kind::ChannelClosed(Sealed))
    }

    #[cfg(any(feature = "http1", feature = "http2", feature = "stream"))]
    pub(super) fn new_body<E: Into<Cause>>(cause: E) -> Error {
        Error::new(Kind::Body(Sealed)).with(cause)
    }

    #[cfg(any(feature = "http1", feature = "http2"))]
    pub(super) fn new_body_write<E: Into<Cause>>(cause: E) -> Error {
        Error::new(Kind::BodyWrite(Sealed)).with(cause)
    }

    pub(super) fn new_body_write_aborted() -> Error {
        Error::new(Kind::BodyWriteAborted(Sealed))
    }

    fn new_user(user: User) -> Error {
        Error::new(Kind::User(user))
    }

    #[cfg(feature = "http1")]
    #[cfg(feature = "server")]
    pub(super) fn new_user_header() -> Error {
        Error::new_user(User::UnexpectedHeader(Sealed))
    }

    #[cfg(any(feature = "http1", feature = "http2"))]
    #[cfg(feature = "client")]
    pub(super) fn new_user_unsupported_version() -> Error {
        Error::new_user(User::UnsupportedVersion(Sealed))
    }

    #[cfg(any(feature = "http1", feature = "http2"))]
    #[cfg(feature = "client")]
    pub(super) fn new_user_unsupported_request_method() -> Error {
        Error::new_user(User::UnsupportedRequestMethod(Sealed))
    }

    #[cfg(feature = "http1")]
    #[cfg(feature = "server")]
    pub(super) fn new_user_unsupported_status_code() -> Error {
        Error::new_user(User::UnsupportedStatusCode(Sealed))
    }

    #[cfg(any(feature = "http1", feature = "http2"))]
    #[cfg(feature = "client")]
    pub(super) fn new_user_absolute_uri_required() -> Error {
        Error::new_user(User::AbsoluteUriRequired(Sealed))
    }

    pub(super) fn new_user_no_upgrade() -> Error {
        Error::new_user(User::NoUpgrade(Sealed))
    }

    #[cfg(feature = "http1")]
    pub(super) fn new_user_manual_upgrade() -> Error {
        Error::new_user(User::ManualUpgrade(Sealed))
    }

    #[cfg(any(feature = "http1", feature = "http2"))]
    #[cfg(feature = "server")]
    pub(super) fn new_user_make_service<E: Into<Cause>>(cause: E) -> Error {
        Error::new_user(User::MakeService(Sealed)).with(cause)
    }

    #[cfg(any(feature = "http1", feature = "http2"))]
    pub(super) fn new_user_service<E: Into<Cause>>(cause: E) -> Error {
        Error::new_user(User::Service(Sealed)).with(cause)
    }

    #[cfg(any(feature = "http1", feature = "http2"))]
    pub(super) fn new_user_body<E: Into<Cause>>(cause: E) -> Error {
        Error::new_user(User::Body(Sealed)).with(cause)
    }

    #[cfg(feature = "http1")]
    pub(super) fn new_shutdown(cause: std::io::Error) -> Error {
        Error::new(Kind::Shutdown(Sealed)).with(cause)
    }

    #[cfg(feature = "ffi")]
    pub(super) fn new_user_aborted_by_callback() -> Error {
        Error::new_user(User::AbortedByCallback(Sealed))
    }

    #[cfg(feature = "http2")]
    pub(super) fn new_h2(cause: ::h2::Error) -> Error {
        if cause.is_io() {
            Error::new_io(cause.into_io().expect("h2::Error::is_io"))
        } else {
            Error::new_fake_h2(cause)
        }
    }

    #[cfg(feature = "http2")]
    pub(super) fn new_fake_h2<E: Into<Cause>>(cause: E) -> Error {
        Error::new(Kind::Http2(Sealed)).with(cause)
    }

    fn description(&self) -> &str {
        match self.inner.kind {
            Kind::Parse(Parse::Method(_)) => "invalid HTTP method parsed",
            Kind::Parse(Parse::Version(_)) => "invalid HTTP version parsed",
            #[cfg(feature = "http1")]
            Kind::Parse(Parse::H2Preface(_)) => "invalid HTTP version parsed (found HTTP2 preface)",
            Kind::Parse(Parse::Uri(_)) => "invalid URI",
            Kind::Parse(Parse::Header(_)) => "invalid HTTP header parsed",
            Kind::Parse(Parse::HeaderSectionTooLarge(_)) => "header section is too large",
            Kind::Parse(Parse::Status(_)) => "invalid HTTP status-code parsed",
            Kind::IncompleteMessage(_) => "connection closed before message completed",
            #[cfg(feature = "http1")]
            Kind::UnexpectedMessage(_) => "received unexpected message from connection",
            Kind::ChannelClosed(_) => "channel closed",
            Kind::Connect(_) => "error trying to connect",
            Kind::Canceled(_) => "operation was canceled",
            #[cfg(all(any(feature = "http1", feature = "http2"), feature = "tcp"))]
            #[cfg(feature = "server")]
            Kind::Listen(_) => "error creating server listener",
            #[cfg(any(feature = "http1", feature = "http2"))]
            #[cfg(feature = "server")]
            Kind::Accept(_) => "error accepting connection",
            #[cfg(any(feature = "http1", feature = "http2", feature = "stream"))]
            Kind::Body(_) => "error reading a body from connection",
            #[cfg(any(feature = "http1", feature = "http2"))]
            Kind::BodyWrite(_) => "error writing a body to connection",
            Kind::BodyWriteAborted(_) => "body write aborted",
            #[cfg(feature = "http1")]
            Kind::Shutdown(_) => "error shutting down connection",
            #[cfg(feature = "http2")]
            Kind::Http2(_) => "http2 error",
            #[cfg(any(feature = "http1", feature = "http2"))]
            Kind::Io(_) => "connection error",

            #[cfg(any(feature = "http1", feature = "http2"))]
            Kind::User(User::Body(_)) => "error from user's HttpBody stream",
            #[cfg(any(feature = "http1", feature = "http2"))]
            #[cfg(feature = "server")]
            Kind::User(User::MakeService(_)) => "error from user's MakeService",
            #[cfg(any(feature = "http1", feature = "http2"))]
            Kind::User(User::Service(_)) => "error from user's Service",
            #[cfg(feature = "http1")]
            #[cfg(feature = "server")]
            Kind::User(User::UnexpectedHeader(_)) => "user sent unexpected header",
            #[cfg(any(feature = "http1", feature = "http2"))]
            #[cfg(feature = "client")]
            Kind::User(User::UnsupportedVersion(_)) => "request has unsupported HTTP version",
            #[cfg(any(feature = "http1", feature = "http2"))]
            #[cfg(feature = "client")]
            Kind::User(User::UnsupportedRequestMethod(_)) => "request has unsupported HTTP method",
            #[cfg(feature = "http1")]
            #[cfg(feature = "server")]
            Kind::User(User::UnsupportedStatusCode(_)) => {
                "response has 1xx status code, not supported by server"
            }
            #[cfg(any(feature = "http1", feature = "http2"))]
            #[cfg(feature = "client")]
            Kind::User(User::AbsoluteUriRequired(_)) => "client requires absolute-form URIs",
            Kind::User(User::NoUpgrade(_)) => "no upgrade available",
            #[cfg(feature = "http1")]
            Kind::User(User::ManualUpgrade(_)) => "upgrade expected but low level API in use",
            #[cfg(feature = "ffi")]
            Kind::User(User::AbortedByCallback(_)) => {
                "operation aborted by an application callback"
            }
        }
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut f = f.debug_tuple("hyper::Error");
        f.field(&self.inner.kind);
        if let Some(ref cause) = self.inner.cause {
            f.field(cause);
        }
        f.finish()
    }
}

impl fmt::Debug for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match *self {
            Self::Parse(ref e) => return f.debug_tuple("Parse").field(e).finish(),
            Self::User(ref e) => return f.debug_tuple("User").field(e).finish(),
            Self::IncompleteMessage(_) => "IncompleteMessage",
            #[cfg(feature = "http1")]
            Self::UnexpectedMessage(_) => "UnexpectedMessage",
            Self::Canceled(_) => "Canceled",
            Self::ChannelClosed(_) => "ChannelClosed",
            #[cfg(any(feature = "http1", feature = "http2"))]
            Self::Io(_) => "Io",
            Self::Connect(_) => "Connect",
            #[cfg(all(
                any(feature = "http1", feature = "http2"),
                feature = "tcp",
                feature = "server"
            ))]
            Self::Listen(_) => "Listen",
            #[cfg(any(feature = "http1", feature = "http2"))]
            #[cfg(feature = "server")]
            Self::Accept(_) => "Accept",
            #[cfg(any(feature = "http1", feature = "http2", feature = "stream"))]
            Self::Body(_) => "Body",
            #[cfg(any(feature = "http1", feature = "http2"))]
            Self::BodyWrite(_) => "BodyWrite",
            Self::BodyWriteAborted(_) => "BodyWriteAborted",
            #[cfg(feature = "http1")]
            Self::Shutdown(_) => "Shutdown",

            #[cfg(feature = "http2")]
            Self::Http2(_) => "Http2",
        })
    }
}

impl fmt::Debug for Parse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match *self {
            Self::Method(_) => "Method",
            Self::Version(_) => "Version",
            #[cfg(feature = "http1")]
            Self::H2Preface(_) => "H2Preface",
            Self::Uri(_) => "Uri",
            Self::Header(_) => "Header",
            Self::HeaderSectionTooLarge(_) => "TooLarge",
            Self::Status(_) => "Status",
        })
    }
}

impl fmt::Debug for User {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match *self {
            #[cfg(any(feature = "http1", feature = "http2"))]
            Self::Body(_) => "Body",
            #[cfg(any(feature = "http1", feature = "http2"))]
            #[cfg(feature = "server")]
            Self::MakeService(_) => "MakeService",
            #[cfg(any(feature = "http1", feature = "http2"))]
            Self::Service(_) => "Service",
            #[cfg(feature = "http1")]
            #[cfg(feature = "server")]
            Self::UnexpectedHeader(_) => "UnexpectedHeader",
            #[cfg(any(feature = "http1", feature = "http2"))]
            #[cfg(feature = "client")]
            Self::UnsupportedVersion(_) => "UnsupportedVersion",
            #[cfg(any(feature = "http1", feature = "http2"))]
            #[cfg(feature = "client")]
            Self::UnsupportedRequestMethod(_) => "UnsupportedRequestMethod",
            #[cfg(feature = "http1")]
            #[cfg(feature = "server")]
            Self::UnsupportedStatusCode(_) => "UnsupportedStatusCode",
            #[cfg(any(feature = "http1", feature = "http2"))]
            #[cfg(feature = "client")]
            Self::AbsoluteUriRequired(_) => "AbsoluteUriRequired",
            Self::NoUpgrade(_) => "NoUpgrade",
            #[cfg(feature = "http1")]
            Self::ManualUpgrade(_) => "ManualUpgrade",
            #[cfg(feature = "ffi")]
            Self::AbortedByCallback(_) => "AbortedByCallback",
        })
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(ref cause) = self.inner.cause {
            write!(f, "{}: {}", self.description(), cause)
        } else {
            f.write_str(self.description())
        }
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.inner
            .cause
            .as_ref()
            .map(|cause| &**cause as &(dyn StdError + 'static))
    }
}

impl Parse {
    pub(super) fn new_method() -> Self {
        Parse::Method(Sealed)
    }

    pub(super) fn new_version() -> Self {
        Parse::Version(Sealed)
    }

    #[cfg(feature = "http1")]
    pub(super) fn new_h2_preface() -> Self {
        Parse::H2Preface(Sealed)
    }

    pub(super) fn new_uri() -> Self {
        Parse::Uri(Sealed)
    }

    pub(super) fn new_header() -> Self {
        Parse::Header(Sealed)
    }

    pub(super) fn new_header_section_too_large() -> Self {
        Parse::HeaderSectionTooLarge(Sealed)
    }

    pub(super) fn new_status() -> Self {
        Parse::Status(Sealed)
    }

    pub(super) fn from_httparse(err: httparse::Error) -> Parse {
        match err {
            httparse::Error::HeaderName
            | httparse::Error::HeaderValue
            | httparse::Error::NewLine
            | httparse::Error::Token => Parse::new_header(),
            httparse::Error::Status => Parse::new_status(),
            httparse::Error::TooManyHeaders => Parse::new_header_section_too_large(),
            httparse::Error::Version => Parse::new_version(),
        }
    }

    pub(super) fn from_invalid_method(_: http::method::InvalidMethod) -> Parse {
        Parse::new_method()
    }

    pub(super) fn from_invalid_status_code(_: http::status::InvalidStatusCode) -> Parse {
        Parse::new_status()
    }

    pub(super) fn from_invalid_uri(_: http::uri::InvalidUri) -> Parse {
        Parse::new_uri()
    }
}

#[doc(hidden)]
trait AssertSendSync: Send + Sync + 'static {}
#[doc(hidden)]
impl AssertSendSync for Error {}

// ===== impl TimedOut ====

impl fmt::Display for TimedOut {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("operation timed out")
    }
}

impl StdError for TimedOut {}

mod sealed {
    /// Exists solely to be able to extend error types later.
    #[allow(unreachable_pub)]
    #[derive(Debug, PartialEq)]
    pub struct Sealed;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem;

    #[test]
    fn error_size_of() {
        assert_eq!(mem::size_of::<Error>(), mem::size_of::<usize>());
    }

    #[cfg(feature = "http2")]
    #[test]
    fn h2_reason_unknown() {
        let closed = Error::new_closed();
        assert_eq!(closed.h2_reason(), h2::Reason::INTERNAL_ERROR);
    }

    #[cfg(feature = "http2")]
    #[test]
    fn h2_reason_one_level() {
        let body_err = Error::new_user_body(h2::Error::from(h2::Reason::ENHANCE_YOUR_CALM));
        assert_eq!(body_err.h2_reason(), h2::Reason::ENHANCE_YOUR_CALM);
    }

    #[cfg(feature = "http2")]
    #[test]
    fn h2_reason_nested() {
        let recvd = Error::new_h2(h2::Error::from(h2::Reason::HTTP_1_1_REQUIRED));
        // Suppose a user were proxying the received error
        let svc_err = Error::new_user_service(recvd);
        assert_eq!(svc_err.h2_reason(), h2::Reason::HTTP_1_1_REQUIRED);
    }
}
