//! Error and Result module.
use std::error::Error as StdError;
use std::fmt;

/// Result type often returned from methods that can have hyper `Error`s.
pub type Result<T> = std::result::Result<T, Error>;

type Cause = Box<dyn StdError + Send + Sync>;

/// Represents errors that can occur handling HTTP streams.
///
/// # Formatting
///
/// The `Display` implementation of this type will only print the details of
/// this level of error, even though it may have been caused by another error
/// and contain that error in its source. To print all the relevant
/// information, including the source chain, using something like
/// `std::error::Report`, or equivalent 3rd party types.
///
/// The contents of the formatted error message of this specific `Error` type
/// is unspecified. **You must not depend on it.** The wording and details may
/// change in any version, with the goal of improving error messages.
///
/// # Source
///
/// A `hyper::Error` may be caused by another error. To aid in debugging,
/// those are exposed in `Error::source()` as erased types. While it is
/// possible to check the exact type of the sources, they **can not be depended
/// on**. They may come from private internal dependencies, and are subject to
/// change at any moment.
pub struct Error {
    inner: Box<ErrorImpl>,
}

struct ErrorImpl {
    kind: Kind,
    cause: Option<Cause>,
}

#[derive(Debug)]
pub(super) enum Kind {
    Parse(Parse),
    User(User),
    /// A message reached EOF, but is not complete.
    #[cfg(any(http1_client, http1_server))]
    IncompleteMessage,
    /// A connection received a message (or bytes) when not waiting for one.
    #[cfg(any(http1_client, http1_server))]
    UnexpectedMessage,
    /// A pending item was dropped before ever being processed.
    Canceled,
    /// Indicates a channel (client or body sender) is closed.
    #[cfg(any(http_client, http1_server))]
    ChannelClosed,
    /// An `io::Error` that occurred while trying to read or write to a network stream.
    #[cfg(any(http_client, http_server))]
    Io,
    /// User took too long to send headers
    #[cfg(http1_server)]
    HeaderTimeout,
    /// Error while reading a body from connection.
    #[cfg(any(http_client, http_server))]
    Body,
    /// Error while writing a body to connection.
    #[cfg(any(http_client, http_server))]
    BodyWrite,
    /// Error calling AsyncWrite::shutdown()
    #[cfg(any(http1_client, http1_server))]
    Shutdown,

    /// A general error from h2.
    #[cfg(any(http2_client, http2_server))]
    Http2,
}

#[derive(Debug)]
pub(super) enum Parse {
    Method,
    #[cfg(http1)]
    Version,
    #[cfg(any(http1_client, http1_server))]
    VersionH2,
    Uri,
    #[cfg(http1_server)]
    UriTooLong,
    #[cfg(http1)]
    Header(Header),
    #[cfg(any(http1, http2))]
    #[cfg_attr(http2, allow(unused))]
    TooLarge,
    Status,
    #[cfg(any(http1_client, http1_server))]
    Internal,
}

#[derive(Debug)]
#[cfg(http1)]
pub(super) enum Header {
    Token,
    #[cfg(any(client, server))]
    ContentLengthInvalid,
    #[cfg(server)]
    TransferEncodingInvalid,
    #[cfg(any(client, server))]
    TransferEncodingUnexpected,
}

#[derive(Debug)]
pub(super) enum User {
    /// Error calling user's Body::poll_data().
    #[cfg(any(http_client, http_server))]
    Body,
    /// The user aborted writing of the outgoing body.
    #[cfg(any(http1_client, http1_server, ffi))]
    BodyWriteAborted,
    /// Error from future of user's Service.
    #[cfg(any(http1_client, http_server))]
    Service,
    /// User tried to send a certain header in an unexpected context.
    ///
    /// For example, sending both `content-length` and `transfer-encoding`.
    #[cfg(any(http1, http2))]
    #[cfg(server)]
    UnexpectedHeader,
    /// User tried to respond with a 1xx (not 101) response code.
    #[cfg(http1)]
    #[cfg(server)]
    UnsupportedStatusCode,

    /// User tried polling for an upgrade that doesn't exist.
    NoUpgrade,

    /// User polled for an upgrade, but low-level API is not using upgrades.
    #[cfg(any(http1_client, http1_server))]
    ManualUpgrade,

    /// The dispatch task is gone.
    #[cfg(any(http_client))]
    DispatchGone,

    /// User aborted in an FFI callback.
    #[cfg(ffi)]
    AbortedByCallback,
}

// Sentinel type to indicate the error was caused by a timeout.
#[derive(Debug)]
pub(super) struct TimedOut;

impl Error {
    /// Returns true if this was an HTTP parse error.
    pub fn is_parse(&self) -> bool {
        matches!(self.inner.kind, Kind::Parse(_))
    }

    /// Returns true if this was an HTTP parse error caused by a message that was too large.
    #[cfg(http1_server)]
    pub fn is_parse_too_large(&self) -> bool {
        matches!(
            self.inner.kind,
            Kind::Parse(Parse::TooLarge) | Kind::Parse(Parse::UriTooLong)
        )
    }

    /// Returns true if this was an HTTP parse error caused by an invalid response status code or
    /// reason phrase.
    pub fn is_parse_status(&self) -> bool {
        matches!(self.inner.kind, Kind::Parse(Parse::Status))
    }

    /// Returns true if this error was caused by user code.
    pub fn is_user(&self) -> bool {
        matches!(self.inner.kind, Kind::User(_))
    }

    /// Returns true if this was about a `Request` that was canceled.
    pub fn is_canceled(&self) -> bool {
        matches!(self.inner.kind, Kind::Canceled)
    }

    /// Returns true if a sender's channel is closed.
    pub fn is_closed(&self) -> bool {
        #[cfg(not(any(http_client, http1_server)))]
        return false;

        #[cfg(any(http_client, http1_server))]
        matches!(self.inner.kind, Kind::ChannelClosed)
    }

    /// Returns true if the connection closed before a message could complete.
    pub fn is_incomplete_message(&self) -> bool {
        #[cfg(not(any(http1_client, http1_server)))]
        return false;

        #[cfg(any(http1_client, http1_server))]
        matches!(self.inner.kind, Kind::IncompleteMessage)
    }

    /// Returns true if the body write was aborted.
    pub fn is_body_write_aborted(&self) -> bool {
        #[cfg(not(any(http1_client, http1_server, ffi)))]
        return false;

        #[cfg(any(http1_client, http1_server, ffi))]
        matches!(self.inner.kind, Kind::User(User::BodyWriteAborted))
    }

    /// Returns true if the error was caused by a timeout.
    pub fn is_timeout(&self) -> bool {
        #[cfg(http1_server)]
        if matches!(self.inner.kind, Kind::HeaderTimeout) {
            return true;
        }
        self.find_source::<TimedOut>().is_some()
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

    #[cfg(any(http1_server, ffi))]
    pub(super) fn kind(&self) -> &Kind {
        &self.inner.kind
    }

    pub(crate) fn find_source<E: StdError + 'static>(&self) -> Option<&E> {
        let mut cause = self.source();
        while let Some(err) = cause {
            if let Some(typed) = err.downcast_ref() {
                return Some(typed);
            }
            cause = err.source();
        }

        // else
        None
    }

    #[cfg(any(http2_client, http2_server))]
    pub(super) fn h2_reason(&self) -> h2::Reason {
        // Find an h2::Reason somewhere in the cause stack, if it exists,
        // otherwise assume an INTERNAL_ERROR.
        self.find_source::<h2::Error>()
            .and_then(|h2_err| h2_err.reason())
            .unwrap_or(h2::Reason::INTERNAL_ERROR)
    }

    pub(super) fn new_canceled() -> Error {
        Error::new(Kind::Canceled)
    }

    #[cfg(any(http1_client, http1_server))]
    pub(super) fn new_incomplete() -> Error {
        Error::new(Kind::IncompleteMessage)
    }

    #[cfg(any(http1_client, http1_server))]
    pub(super) fn new_too_large() -> Error {
        Error::new(Kind::Parse(Parse::TooLarge))
    }

    #[cfg(any(http1_client, http1_server))]
    pub(super) fn new_version_h2() -> Error {
        Error::new(Kind::Parse(Parse::VersionH2))
    }

    #[cfg(any(http1_client, http1_server))]
    pub(super) fn new_unexpected_message() -> Error {
        Error::new(Kind::UnexpectedMessage)
    }

    #[cfg(any(http_client, http_server))]
    pub(super) fn new_io(cause: std::io::Error) -> Error {
        Error::new(Kind::Io).with(cause)
    }

    #[cfg(any(http_client, http1_server))]
    pub(super) fn new_closed() -> Error {
        Error::new(Kind::ChannelClosed)
    }

    #[cfg(any(http_client, http_server))]
    pub(super) fn new_body<E: Into<Cause>>(cause: E) -> Error {
        Error::new(Kind::Body).with(cause)
    }

    #[cfg(any(http_client, http_server))]
    pub(super) fn new_body_write<E: Into<Cause>>(cause: E) -> Error {
        Error::new(Kind::BodyWrite).with(cause)
    }

    #[cfg(any(http1_client, http1_server, ffi))]
    pub(super) fn new_body_write_aborted() -> Error {
        Error::new(Kind::User(User::BodyWriteAborted))
    }

    fn new_user(user: User) -> Error {
        Error::new(Kind::User(user))
    }

    #[cfg(any(http1, http2))]
    #[cfg(server)]
    pub(super) fn new_user_header() -> Error {
        Error::new_user(User::UnexpectedHeader)
    }

    #[cfg(http1_server)]
    pub(super) fn new_header_timeout() -> Error {
        Error::new(Kind::HeaderTimeout)
    }

    #[cfg(http1)]
    #[cfg(server)]
    pub(super) fn new_user_unsupported_status_code() -> Error {
        Error::new_user(User::UnsupportedStatusCode)
    }

    pub(super) fn new_user_no_upgrade() -> Error {
        Error::new_user(User::NoUpgrade)
    }

    #[cfg(any(http1_client, http1_server))]
    pub(super) fn new_user_manual_upgrade() -> Error {
        Error::new_user(User::ManualUpgrade)
    }

    #[cfg(any(http1_client, http_server))]
    pub(super) fn new_user_service<E: Into<Cause>>(cause: E) -> Error {
        Error::new_user(User::Service).with(cause)
    }

    #[cfg(any(http_client, http_server))]
    pub(super) fn new_user_body<E: Into<Cause>>(cause: E) -> Error {
        Error::new_user(User::Body).with(cause)
    }

    #[cfg(any(http1_client, http1_server))]
    pub(super) fn new_shutdown(cause: std::io::Error) -> Error {
        Error::new(Kind::Shutdown).with(cause)
    }

    #[cfg(ffi)]
    pub(super) fn new_user_aborted_by_callback() -> Error {
        Error::new_user(User::AbortedByCallback)
    }

    #[cfg(http_client)]
    pub(super) fn new_user_dispatch_gone() -> Error {
        Error::new(Kind::User(User::DispatchGone))
    }

    #[cfg(any(http2_client, http2_server))]
    pub(super) fn new_h2(cause: ::h2::Error) -> Error {
        if cause.is_io() {
            Error::new_io(cause.into_io().expect("h2::Error::is_io"))
        } else {
            Error::new(Kind::Http2).with(cause)
        }
    }

    fn description(&self) -> &str {
        match self.inner.kind {
            Kind::Parse(Parse::Method) => "invalid HTTP method parsed",
            #[cfg(http1)]
            Kind::Parse(Parse::Version) => "invalid HTTP version parsed",
            #[cfg(any(http1_client, http1_server))]
            Kind::Parse(Parse::VersionH2) => "invalid HTTP version parsed (found HTTP2 preface)",
            Kind::Parse(Parse::Uri) => "invalid URI",
            #[cfg(http1_server)]
            Kind::Parse(Parse::UriTooLong) => "URI too long",
            #[cfg(http1)]
            Kind::Parse(Parse::Header(Header::Token)) => "invalid HTTP header parsed",
            #[cfg(any(http1_client, http1_server))]
            Kind::Parse(Parse::Header(Header::ContentLengthInvalid)) => {
                "invalid content-length parsed"
            }
            #[cfg(http1_server)]
            Kind::Parse(Parse::Header(Header::TransferEncodingInvalid)) => {
                "invalid transfer-encoding parsed"
            }
            #[cfg(any(http1_client, http1_server))]
            Kind::Parse(Parse::Header(Header::TransferEncodingUnexpected)) => {
                "unexpected transfer-encoding parsed"
            }
            #[cfg(any(http1, http2))]
            Kind::Parse(Parse::TooLarge) => "message head is too large",
            Kind::Parse(Parse::Status) => "invalid HTTP status-code parsed",
            #[cfg(any(http1_client, http1_server))]
            Kind::Parse(Parse::Internal) => {
                "internal error inside Hyper and/or its dependencies, please report"
            }
            #[cfg(any(http1_client, http1_server))]
            Kind::IncompleteMessage => "connection closed before message completed",
            #[cfg(any(http1_client, http1_server))]
            Kind::UnexpectedMessage => "received unexpected message from connection",
            #[cfg(any(http_client, http1_server))]
            Kind::ChannelClosed => "channel closed",
            Kind::Canceled => "operation was canceled",
            #[cfg(http1_server)]
            Kind::HeaderTimeout => "read header from client timeout",
            #[cfg(any(http_client, http_server))]
            Kind::Body => "error reading a body from connection",
            #[cfg(any(http_client, http_server))]
            Kind::BodyWrite => "error writing a body to connection",
            #[cfg(any(http1_client, http1_server))]
            Kind::Shutdown => "error shutting down connection",
            #[cfg(any(http2_client, http2_server))]
            Kind::Http2 => "http2 error",
            #[cfg(any(http_client, http_server))]
            Kind::Io => "connection error",

            #[cfg(any(http_client, http_server))]
            Kind::User(User::Body) => "error from user's Body stream",
            #[cfg(any(http1_client, http1_server, ffi))]
            Kind::User(User::BodyWriteAborted) => "user body write aborted",
            #[cfg(any(http_client, http1_server))]
            Kind::User(User::Service) => "error from user's Service",
            #[cfg(any(http1, http2))]
            #[cfg(server)]
            Kind::User(User::UnexpectedHeader) => "user sent unexpected header",
            #[cfg(http1)]
            #[cfg(server)]
            Kind::User(User::UnsupportedStatusCode) => {
                "response has 1xx status code, not supported by server"
            }
            Kind::User(User::NoUpgrade) => "no upgrade available",
            #[cfg(any(http1_client, http1_server))]
            Kind::User(User::ManualUpgrade) => "upgrade expected but low level API in use",
            #[cfg(http_client)]
            Kind::User(User::DispatchGone) => "dispatch task is gone",
            #[cfg(ffi)]
            Kind::User(User::AbortedByCallback) => "operation aborted by an application callback",
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

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.description())
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

#[doc(hidden)]
impl From<Parse> for Error {
    fn from(err: Parse) -> Error {
        Error::new(Kind::Parse(err))
    }
}

#[cfg(http1)]
impl Parse {
    #[cfg(any(client, server))]
    pub(crate) fn content_length_invalid() -> Self {
        Parse::Header(Header::ContentLengthInvalid)
    }

    #[cfg(server)]
    pub(crate) fn transfer_encoding_invalid() -> Self {
        Parse::Header(Header::TransferEncodingInvalid)
    }

    #[cfg(any(client, server))]
    pub(crate) fn transfer_encoding_unexpected() -> Self {
        Parse::Header(Header::TransferEncodingUnexpected)
    }
}

#[cfg(http1)]
impl From<httparse::Error> for Parse {
    fn from(err: httparse::Error) -> Parse {
        match err {
            httparse::Error::HeaderName
            | httparse::Error::HeaderValue
            | httparse::Error::NewLine
            | httparse::Error::Token => Parse::Header(Header::Token),
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

impl From<http::uri::InvalidUriParts> for Parse {
    fn from(_: http::uri::InvalidUriParts) -> Parse {
        Parse::Uri
    }
}

// ===== impl TimedOut ====

impl fmt::Display for TimedOut {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("operation timed out")
    }
}

impl StdError for TimedOut {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem;

    fn assert_send_sync<T: Send + Sync + 'static>() {}

    #[test]
    fn error_satisfies_send_sync() {
        assert_send_sync::<Error>()
    }

    #[test]
    fn error_size_of() {
        assert_eq!(mem::size_of::<Error>(), mem::size_of::<usize>());
    }

    #[cfg(http2)]
    #[test]
    fn h2_reason_unknown() {
        let closed = Error::new_closed();
        assert_eq!(closed.h2_reason(), h2::Reason::INTERNAL_ERROR);
    }

    #[cfg(http2)]
    #[test]
    fn h2_reason_one_level() {
        let body_err = Error::new_user_body(h2::Error::from(h2::Reason::ENHANCE_YOUR_CALM));
        assert_eq!(body_err.h2_reason(), h2::Reason::ENHANCE_YOUR_CALM);
    }

    #[cfg(http2)]
    #[test]
    fn h2_reason_nested() {
        let recvd = Error::new_h2(h2::Error::from(h2::Reason::HTTP_1_1_REQUIRED));
        // Suppose a user were proxying the received error
        let svc_err = Error::new_user_service(recvd);
        assert_eq!(svc_err.h2_reason(), h2::Reason::HTTP_1_1_REQUIRED);
    }
}
