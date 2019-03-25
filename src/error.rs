//! Error and Result module.
use std::error::Error as StdError;
use std::fmt;
use std::io;

use httparse;
use http;
use h2;

/// Result type often returned from methods that can have hyper `Error`s.
pub type Result<T> = ::std::result::Result<T, Error>;

type Cause = Box<StdError + Send + Sync>;

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
    /// A message reached EOF, but is not complete.
    Incomplete,
    /// A client connection received a response when not waiting for one.
    MismatchedResponse,
    /// A pending item was dropped before ever being processed.
    Canceled,
    /// Indicates a connection is closed.
    Closed,
    /// An `io::Error` that occurred while trying to read or write to a network stream.
    Io,
    /// Error occurred while connecting.
    Connect,
    /// Error creating a TcpListener.
    #[cfg(feature = "runtime")]
    Listen,
    /// Error accepting on an Incoming stream.
    Accept,
    /// Error calling user's NewService::new_service().
    NewService,
    /// Error from future of user's Service::call().
    Service,
    /// Error while reading a body from connection.
    Body,
    /// Error while writing a body to connection.
    BodyWrite,
    /// Error calling user's Payload::poll_data().
    BodyUser,
    /// Error calling AsyncWrite::shutdown()
    Shutdown,

    /// A general error from h2.
    Http2,

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

/*
#[derive(Debug)]
pub(crate) enum User {
    VersionNotSupported,
    MethodNotSupported,
    InvalidRequestUri,
}
*/

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
            Kind::BodyUser |
            Kind::NewService |
            Kind::Service |
            Kind::Closed |
            Kind::UnsupportedVersion |
            Kind::UnsupportedRequestMethod |
            Kind::UnsupportedStatusCode |
            Kind::AbsoluteUriRequired |
            Kind::NoUpgrade |
            Kind::Execute => true,
            _ => false,
        }
    }

    /// Returns true if this was about a `Request` that was canceled.
    pub fn is_canceled(&self) -> bool {
        self.inner.kind == Kind::Canceled
    }

    /// Returns true if a sender's channel is closed.
    pub fn is_closed(&self) -> bool {
        self.inner.kind == Kind::Closed
    }

    /// Returns true if this was an error from `Connect`.
    pub fn is_connect(&self) -> bool {
        self.inner.kind == Kind::Connect
    }

    #[doc(hidden)]
    #[cfg_attr(error_source, deprecated(note = "use Error::source instead"))]
    pub fn cause2(&self) -> Option<&(StdError + 'static + Sync + Send)> {
        self.inner.cause.as_ref().map(|e| &**e)
    }

    /// Consumes the error, returning its cause.
    pub fn into_cause(self) -> Option<Box<StdError + Sync + Send>> {
        self.inner.cause
    }

    pub(crate) fn new(kind: Kind, cause: Option<Cause>) -> Error {
        Error {
            inner: Box::new(ErrorImpl {
                kind,
                cause,
            }),
        }
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

    pub(crate) fn new_canceled<E: Into<Cause>>(cause: Option<E>) -> Error {
        Error::new(Kind::Canceled, cause.map(Into::into))
    }

    pub(crate) fn new_incomplete() -> Error {
        Error::new(Kind::Incomplete, None)
    }

    pub(crate) fn new_too_large() -> Error {
        Error::new(Kind::Parse(Parse::TooLarge), None)
    }

    pub(crate) fn new_header() -> Error {
        Error::new(Kind::Parse(Parse::Header), None)
    }

    pub(crate) fn new_version_h2() -> Error {
        Error::new(Kind::Parse(Parse::VersionH2), None)
    }

    pub(crate) fn new_mismatched_response() -> Error {
        Error::new(Kind::MismatchedResponse, None)
    }

    pub(crate) fn new_io(cause: io::Error) -> Error {
        Error::new(Kind::Io, Some(cause.into()))
    }

    #[cfg(feature = "runtime")]
    pub(crate) fn new_listen<E: Into<Cause>>(cause: E) -> Error {
        Error::new(Kind::Listen, Some(cause.into()))
    }

    pub(crate) fn new_accept<E: Into<Cause>>(cause: E) -> Error {
        Error::new(Kind::Accept, Some(cause.into()))
    }

    pub(crate) fn new_connect<E: Into<Cause>>(cause: E) -> Error {
        Error::new(Kind::Connect, Some(cause.into()))
    }

    pub(crate) fn new_closed() -> Error {
        Error::new(Kind::Closed, None)
    }

    pub(crate) fn new_body<E: Into<Cause>>(cause: E) -> Error {
        Error::new(Kind::Body, Some(cause.into()))
    }

    pub(crate) fn new_body_write<E: Into<Cause>>(cause: E) -> Error {
        Error::new(Kind::BodyWrite, Some(cause.into()))
    }

    pub(crate) fn new_user_unsupported_version() -> Error {
        Error::new(Kind::UnsupportedVersion, None)
    }

    pub(crate) fn new_user_unsupported_request_method() -> Error {
        Error::new(Kind::UnsupportedRequestMethod, None)
    }

    pub(crate) fn new_user_unsupported_status_code() -> Error {
        Error::new(Kind::UnsupportedStatusCode, None)
    }

    pub(crate) fn new_user_absolute_uri_required() -> Error {
        Error::new(Kind::AbsoluteUriRequired, None)
    }

    pub(crate) fn new_user_no_upgrade() -> Error {
        Error::new(Kind::NoUpgrade, None)
    }

    pub(crate) fn new_user_manual_upgrade() -> Error {
        Error::new(Kind::ManualUpgrade, None)
    }

    pub(crate) fn new_user_new_service<E: Into<Cause>>(cause: E) -> Error {
        Error::new(Kind::NewService, Some(cause.into()))
    }

    pub(crate) fn new_user_service<E: Into<Cause>>(cause: E) -> Error {
        Error::new(Kind::Service, Some(cause.into()))
    }

    pub(crate) fn new_user_body<E: Into<Cause>>(cause: E) -> Error {
        Error::new(Kind::BodyUser, Some(cause.into()))
    }

    pub(crate) fn new_shutdown(cause: io::Error) -> Error {
        Error::new(Kind::Shutdown, Some(Box::new(cause)))
    }

    pub(crate) fn new_execute<E: Into<Cause>>(cause: E) -> Error {
        Error::new(Kind::Execute, Some(cause.into()))
    }

    pub(crate) fn new_h2(cause: ::h2::Error) -> Error {
        Error::new(Kind::Http2, Some(Box::new(cause)))
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut f = f.debug_struct("Error");
        f.field("kind", &self.inner.kind);
        if let Some(ref cause) = self.inner.cause {
            f.field("cause", cause);
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
            Kind::Parse(Parse::Method) => "invalid Method specified",
            Kind::Parse(Parse::Version) => "invalid HTTP version specified",
            Kind::Parse(Parse::VersionH2) => "invalid HTTP version specified (Http2)",
            Kind::Parse(Parse::Uri) => "invalid URI",
            Kind::Parse(Parse::Header) => "invalid Header provided",
            Kind::Parse(Parse::TooLarge) => "message head is too large",
            Kind::Parse(Parse::Status) => "invalid Status provided",
            Kind::Incomplete => "parsed HTTP message from remote is incomplete",
            Kind::MismatchedResponse => "response received without matching request",
            Kind::Closed => "connection closed",
            Kind::Connect => "an error occurred trying to connect",
            Kind::Canceled => "an operation was canceled internally before starting",
            #[cfg(feature = "runtime")]
            Kind::Listen => "error creating server listener",
            Kind::Accept => "error accepting connection",
            Kind::NewService => "calling user's new_service failed",
            Kind::Service => "error from user's server service",
            Kind::Body => "error reading a body from connection",
            Kind::BodyWrite => "error writing a body to connection",
            Kind::BodyUser => "error from user's Payload stream",
            Kind::Shutdown => "error shutting down connection",
            Kind::Http2 => "http2 general error",
            Kind::UnsupportedVersion => "request has unsupported HTTP version",
            Kind::UnsupportedRequestMethod => "request has unsupported HTTP method",
            Kind::UnsupportedStatusCode => "response has 1xx status code, not supported by server",
            Kind::AbsoluteUriRequired => "client requires absolute-form URIs",
            Kind::NoUpgrade => "no upgrade available",
            Kind::ManualUpgrade => "upgrade expected but low level API in use",
            Kind::Execute => "executor failed to spawn task",

            Kind::Io => "an IO error occurred",
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
    fn source(&self) -> Option<&(StdError + 'static)> {
        self
            .inner
            .cause
            .as_ref()
            .map(|cause| &**cause as &(StdError + 'static))
    }
}

#[doc(hidden)]
impl From<Parse> for Error {
    fn from(err: Parse) -> Error {
        Error::new(Kind::Parse(err), None)
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
    use super::*;

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
