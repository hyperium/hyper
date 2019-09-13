//! Error and Result module.
use std::error::Error as StdError;
use std::fmt;
use std::io;

use httparse;
use http;
use h2;

/// Result type often returned from methods that can have hyper `Error`s.
pub type Result<T> = ::std::result::Result<T, Error>;

type Cause = Box<dyn StdError + Send + Sync>;

/// Represents errors that can occur handling HTTP streams.
pub struct Error {
    inner: Box<ErrorImpl>,
}

struct ErrorImpl {
    kind: Kind,
    cause: Option<Cause>,
}

#[derive(Debug, PartialEq)]
pub(crate) enum Kind {
    Parse(Parse),
    User(User),
    /// A message reached EOF, but is not complete.
    IncompleteMessage,
    /// A connection received a message (or bytes) when not waiting for one.
    UnexpectedMessage,
    /// A pending item was dropped before ever being processed.
    Canceled,
    /// Indicates a channel (client or body sender) is closed.
    ChannelClosed,
    /// An `io::Error` that occurred while trying to read or write to a network stream.
    Io,
    /// Error occurred while connecting.
    Connect,
    /// Error creating a TcpListener.
    #[cfg(feature = "runtime")]
    Listen,
    /// Error accepting on an Incoming stream.
    Accept,
    /// Error while reading a body from connection.
    Body,
    /// Error while writing a body to connection.
    BodyWrite,
    /// The body write was aborted.
    BodyWriteAborted,
    /// Error calling AsyncWrite::shutdown()
    Shutdown,

    /// A general error from h2.
    Http2,
}

#[derive(Debug, PartialEq)]
pub(crate) enum Parse {
    Method,
    Version,
    VersionH2,
    Uri,
    Header,
    TooLarge,
    Status,
}

#[derive(Debug, PartialEq)]
pub(crate) enum User {
    /// Error calling user's Payload::poll_data().
    Body,
    /// Error calling user's MakeService.
    MakeService,
    /// Error from future of user's Service.
    Service,
    /// User tried to send a certain header in an unexpected context.
    ///
    /// For example, sending both `content-length` and `transfer-encoding`.
    UnexpectedHeader,
    /// User tried to create a Request with bad version.
    UnsupportedVersion,
    /// User tried to create a CONNECT Request with the Client.
    UnsupportedRequestMethod,
    /// User tried to respond with a 1xx (not 101) response code.
    UnsupportedStatusCode,
    /// User tried to send a Request with Client with non-absolute URI.
    AbsoluteUriRequired,

    /// User tried polling for an upgrade that doesn't exist.
    NoUpgrade,

    /// User polled for an upgrade, but low-level API is not using upgrades.
    ManualUpgrade,

    /// Error trying to call `Executor::execute`.
    Execute,
}

impl Error {
    /// Returns true if this was an HTTP parse error.
    pub fn is_parse(&self) -> bool {
        match self.inner.kind {
            Kind::Parse(_) => true,
            _ => false,
        }
    }

    /// Returns true if this error was caused by user code.
    pub fn is_user(&self) -> bool {
        match self.inner.kind {
            Kind::User(_) => true,
            _ => false,
        }
    }

    /// Returns true if this was about a `Request` that was canceled.
    pub fn is_canceled(&self) -> bool {
        self.inner.kind == Kind::Canceled
    }

    /// Returns true if a sender's channel is closed.
    pub fn is_closed(&self) -> bool {
        self.inner.kind == Kind::ChannelClosed
    }

    /// Returns true if this was an error from `Connect`.
    pub fn is_connect(&self) -> bool {
        self.inner.kind == Kind::Connect
    }

    /// Returns true if the connection closed before a message could complete.
    pub fn is_incomplete_message(&self) -> bool {
        self.inner.kind == Kind::IncompleteMessage
    }

    /// Returns true if the body write was aborted.
    pub fn is_body_write_aborted(&self) -> bool {
        self.inner.kind == Kind::BodyWriteAborted
    }

    #[doc(hidden)]
    #[cfg_attr(error_source, deprecated(note = "use Error::source instead"))]
    pub fn cause2(&self) -> Option<&(dyn StdError + 'static + Sync + Send)> {
        self.inner.cause.as_ref().map(|e| &**e)
    }

    /// Consumes the error, returning its cause.
    pub fn into_cause(self) -> Option<Box<dyn StdError + Sync + Send>> {
        self.inner.cause
    }

    pub(crate) fn new(kind: Kind) -> Error {
        Error {
            inner: Box::new(ErrorImpl {
                kind,
                cause: None,
            }),
        }
    }

    pub(crate) fn with<C: Into<Cause>>(mut self, cause: C) -> Error {
        self.inner.cause = Some(cause.into());
        self
    }

    pub(crate) fn kind(&self) -> &Kind {
        &self.inner.kind
    }

    #[cfg(not(error_source))]
    pub(crate) fn h2_reason(&self) -> h2::Reason {
        // Since we don't have access to `Error::source`, we can only
        // look so far...
        let mut cause = self.cause2();
        while let Some(err) = cause {
            if let Some(h2_err) = err.downcast_ref::<h2::Error>() {
                return h2_err
                    .reason()
                    .unwrap_or(h2::Reason::INTERNAL_ERROR);
            }

            cause = err
                .downcast_ref::<Error>()
                .and_then(Error::cause2);
        }

        // else
        h2::Reason::INTERNAL_ERROR
    }

    #[cfg(error_source)]
    pub(crate) fn h2_reason(&self) -> h2::Reason {
        // Find an h2::Reason somewhere in the cause stack, if it exists,
        // otherwise assume an INTERNAL_ERROR.
        let mut cause = self.source();
        while let Some(err) = cause {
            if let Some(h2_err) = err.downcast_ref::<h2::Error>() {
                return h2_err
                    .reason()
                    .unwrap_or(h2::Reason::INTERNAL_ERROR);
            }
            cause = err.source();
        }

        // else
        h2::Reason::INTERNAL_ERROR
    }

    pub(crate) fn new_canceled() -> Error {
        Error::new(Kind::Canceled)
    }

    pub(crate) fn new_incomplete() -> Error {
        Error::new(Kind::IncompleteMessage)
    }

    pub(crate) fn new_too_large() -> Error {
        Error::new(Kind::Parse(Parse::TooLarge))
    }

    pub(crate) fn new_version_h2() -> Error {
        Error::new(Kind::Parse(Parse::VersionH2))
    }

    pub(crate) fn new_unexpected_message() -> Error {
        Error::new(Kind::UnexpectedMessage)
    }

    pub(crate) fn new_io(cause: io::Error) -> Error {
        Error::new(Kind::Io).with(cause)
    }

    #[cfg(feature = "runtime")]
    pub(crate) fn new_listen<E: Into<Cause>>(cause: E) -> Error {
        Error::new(Kind::Listen).with(cause)
    }

    pub(crate) fn new_accept<E: Into<Cause>>(cause: E) -> Error {
        Error::new(Kind::Accept).with(cause)
    }

    pub(crate) fn new_connect<E: Into<Cause>>(cause: E) -> Error {
        Error::new(Kind::Connect).with(cause)
    }

    pub(crate) fn new_closed() -> Error {
        Error::new(Kind::ChannelClosed)
    }

    pub(crate) fn new_body<E: Into<Cause>>(cause: E) -> Error {
        Error::new(Kind::Body).with(cause)
    }

    pub(crate) fn new_body_write<E: Into<Cause>>(cause: E) -> Error {
        Error::new(Kind::BodyWrite).with(cause)
    }

    pub(crate) fn new_body_write_aborted() -> Error {
        Error::new(Kind::BodyWriteAborted)
    }

    fn new_user(user: User) -> Error {
        Error::new(Kind::User(user))
    }

    pub(crate) fn new_user_header() -> Error {
        Error::new_user(User::UnexpectedHeader)
    }

    pub(crate) fn new_user_unsupported_version() -> Error {
        Error::new_user(User::UnsupportedVersion)
    }

    pub(crate) fn new_user_unsupported_request_method() -> Error {
        Error::new_user(User::UnsupportedRequestMethod)
    }

    pub(crate) fn new_user_unsupported_status_code() -> Error {
        Error::new_user(User::UnsupportedStatusCode)
    }

    pub(crate) fn new_user_absolute_uri_required() -> Error {
        Error::new_user(User::AbsoluteUriRequired)
    }

    pub(crate) fn new_user_no_upgrade() -> Error {
        Error::new_user(User::NoUpgrade)
    }

    pub(crate) fn new_user_manual_upgrade() -> Error {
        Error::new_user(User::ManualUpgrade)
    }

    pub(crate) fn new_user_make_service<E: Into<Cause>>(cause: E) -> Error {
        Error::new_user(User::MakeService).with(cause)
    }

    pub(crate) fn new_user_service<E: Into<Cause>>(cause: E) -> Error {
        Error::new_user(User::Service).with(cause)
    }

    pub(crate) fn new_user_body<E: Into<Cause>>(cause: E) -> Error {
        Error::new_user(User::Body).with(cause)
    }

    pub(crate) fn new_shutdown(cause: io::Error) -> Error {
        Error::new(Kind::Shutdown).with(cause)
    }

    pub(crate) fn new_execute<E: Into<Cause>>(cause: E) -> Error {
        Error::new_user(User::Execute).with(cause)
    }

    pub(crate) fn new_h2(cause: ::h2::Error) -> Error {
        if cause.is_io() {
            Error::new_io(cause.into_io().expect("h2::Error::is_io"))
        } else {
            Error::new(Kind::Http2).with(cause)
        }
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut f = f.debug_tuple("Error");
        f.field(&self.inner.kind);
        if let Some(ref cause) = self.inner.cause {
            f.field(cause);
        }
        f.finish()
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Some(ref cause) = self.inner.cause {
            write!(f, "{}: {}", self.description(), cause)
        } else {
            f.write_str(self.description())
        }
    }
}

impl StdError for Error {
    fn description(&self) -> &str {
        match self.inner.kind {
            Kind::Parse(Parse::Method) => "invalid HTTP method parsed",
            Kind::Parse(Parse::Version) => "invalid HTTP version parsed",
            Kind::Parse(Parse::VersionH2) => "invalid HTTP version parsed (found HTTP2 preface)",
            Kind::Parse(Parse::Uri) => "invalid URI",
            Kind::Parse(Parse::Header) => "invalid HTTP header parsed",
            Kind::Parse(Parse::TooLarge) => "message head is too large",
            Kind::Parse(Parse::Status) => "invalid HTTP status-code parsed",
            Kind::IncompleteMessage => "connection closed before message completed",
            Kind::UnexpectedMessage => "received unexpected message from connection",
            Kind::ChannelClosed => "channel closed",
            Kind::Connect => "error trying to connect",
            Kind::Canceled => "operation was canceled",
            #[cfg(feature = "runtime")]
            Kind::Listen => "error creating server listener",
            Kind::Accept => "error accepting connection",
            Kind::Body => "error reading a body from connection",
            Kind::BodyWrite => "error writing a body to connection",
            Kind::BodyWriteAborted => "body write aborted",
            Kind::Shutdown => "error shutting down connection",
            Kind::Http2 => "http2 error",
            Kind::Io => "connection error",

            Kind::User(User::Body) => "error from user's Payload stream",
            Kind::User(User::MakeService) => "error from user's MakeService",
            Kind::User(User::Service) => "error from user's Service",
            Kind::User(User::UnexpectedHeader) => "user sent unexpected header",
            Kind::User(User::UnsupportedVersion) => "request has unsupported HTTP version",
            Kind::User(User::UnsupportedRequestMethod) => "request has unsupported HTTP method",
            Kind::User(User::UnsupportedStatusCode) => "response has 1xx status code, not supported by server",
            Kind::User(User::AbsoluteUriRequired) => "client requires absolute-form URIs",
            Kind::User(User::NoUpgrade) => "no upgrade available",
            Kind::User(User::ManualUpgrade) => "upgrade expected but low level API in use",
            Kind::User(User::Execute) => "executor failed to spawn task",
        }
    }

    #[cfg(not(error_source))]
    fn cause(&self) -> Option<&StdError> {
        self
            .inner
            .cause
            .as_ref()
            .map(|cause| &**cause as &StdError)
    }

    #[cfg(error_source)]
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self
            .inner
            .cause
            .as_ref()
            .map(|cause| &**cause as &(dyn StdError + 'static))
    }
}

#[doc(hidden)]
impl From<Parse> for Error {
    fn from(err: Parse) -> Error {
        Error::new(Kind::Parse(err))
    }
}

impl From<httparse::Error> for Parse {
    fn from(err: httparse::Error) -> Parse {
        match err {
            httparse::Error::HeaderName |
            httparse::Error::HeaderValue |
            httparse::Error::NewLine |
            httparse::Error::Token => Parse::Header,
            httparse::Error::Status => Parse::Status,
            httparse::Error::TooManyHeaders => Parse::TooLarge,
            httparse::Error::Version => Parse::Version,
        }
    }
}

impl From<http::method::InvalidMethod> for Parse {
    fn from(_: http::method::InvalidMethod) -> Parse {
        Parse::Method
    }
}

impl From<http::status::InvalidStatusCode> for Parse {
    fn from(_: http::status::InvalidStatusCode) -> Parse {
        Parse::Status
    }
}

impl From<http::uri::InvalidUri> for Parse {
    fn from(_: http::uri::InvalidUri) -> Parse {
        Parse::Uri
    }
}

impl From<http::uri::InvalidUriBytes> for Parse {
    fn from(_: http::uri::InvalidUriBytes) -> Parse {
        Parse::Uri
    }
}

impl From<http::uri::InvalidUriParts> for Parse {
    fn from(_: http::uri::InvalidUriParts) -> Parse {
        Parse::Uri
    }
}

#[doc(hidden)]
trait AssertSendSync: Send + Sync + 'static {}
#[doc(hidden)]
impl AssertSendSync for Error {}

#[cfg(test)]
mod tests {
    use std::mem;
    use super::*;

    #[test]
    fn error_size_of() {
        assert_eq!(mem::size_of::<Error>(), mem::size_of::<usize>());
    }

    #[test]
    fn h2_reason_unknown() {
        let closed = Error::new_closed();
        assert_eq!(closed.h2_reason(), h2::Reason::INTERNAL_ERROR);
    }

    #[test]
    fn h2_reason_one_level() {
        let body_err = Error::new_user_body(h2::Error::from(h2::Reason::ENHANCE_YOUR_CALM));
        assert_eq!(body_err.h2_reason(), h2::Reason::ENHANCE_YOUR_CALM);
    }

    #[test]
    fn h2_reason_nested() {
        let recvd = Error::new_h2(h2::Error::from(h2::Reason::HTTP_1_1_REQUIRED));
        // Suppose a user were proxying the received error
        let svc_err = Error::new_user_service(recvd);
        assert_eq!(svc_err.h2_reason(), h2::Reason::HTTP_1_1_REQUIRED);
    }
}
