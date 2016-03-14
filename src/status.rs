//! HTTP status codes
use std::fmt;
use std::cmp::Ordering;

// shamelessly lifted from Teepee. I tried a few schemes, this really
// does seem like the best. Improved scheme to support arbitrary status codes.

/// An HTTP status code (`status-code` in RFC 7230 et al.).
///
/// This enum contains all common status codes and an Unregistered
/// extension variant. It allows status codes in the range [0, 65535], as any
/// `u16` integer may be used as a status code for XHR requests. It is
/// recommended to only use values between [100, 599], since only these are
/// defined as valid status codes with a status class by HTTP.
///
/// If you encounter a status code that you do not know how to deal with, you
/// should treat it as the `x00` status code—e.g. for code 123, treat it as
/// 100 (Continue). This can be achieved with
/// `self.class().default_code()`:
///
/// ```rust
/// # use hyper::status::StatusCode;
/// let status = StatusCode::Unregistered(123);
/// assert_eq!(status.class().default_code(), StatusCode::Continue);
/// ```
///
/// IANA maintain the [Hypertext Transfer Protocol (HTTP) Status Code
/// Registry](http://www.iana.org/assignments/http-status-codes/http-status-codes.xhtml) which is
/// the source for this enum (with one exception, 418 I'm a teapot, which is
/// inexplicably not in the register).
#[derive(Debug, Hash)]
pub enum StatusCode {
    /// 100 Continue
    /// [[RFC7231, Section 6.2.1](https://tools.ietf.org/html/rfc7231#section-6.2.1)]
    Continue,
    /// 101 Switching Protocols
    /// [[RFC7231, Section 6.2.2](https://tools.ietf.org/html/rfc7231#section-6.2.2)]
    SwitchingProtocols,
    /// 102 Processing
    /// [[RFC2518](https://tools.ietf.org/html/rfc2518)]
    Processing,

    /// 200 OK
    /// [[RFC7231, Section 6.3.1](https://tools.ietf.org/html/rfc7231#section-6.3.1)]
    Ok,
    /// 201 Created
    /// [[RFC7231, Section 6.3.2](https://tools.ietf.org/html/rfc7231#section-6.3.2)]
    Created,
    /// 202 Accepted
    /// [[RFC7231, Section 6.3.3](https://tools.ietf.org/html/rfc7231#section-6.3.3)]
    Accepted,
    /// 203 Non-Authoritative Information
    /// [[RFC7231, Section 6.3.4](https://tools.ietf.org/html/rfc7231#section-6.3.4)]
    NonAuthoritativeInformation,
    /// 204 No Content
    /// [[RFC7231, Section 6.3.5](https://tools.ietf.org/html/rfc7231#section-6.3.5)]
    NoContent,
    /// 205 Reset Content
    /// [[RFC7231, Section 6.3.6](https://tools.ietf.org/html/rfc7231#section-6.3.6)]
    ResetContent,
    /// 206 Partial Content
    /// [[RFC7233, Section 4.1](https://tools.ietf.org/html/rfc7233#section-4.1)]
    PartialContent,
    /// 207 Multi-Status
    /// [[RFC4918](https://tools.ietf.org/html/rfc4918)]
    MultiStatus,
    /// 208 Already Reported
    /// [[RFC5842](https://tools.ietf.org/html/rfc5842)]
    AlreadyReported,

    /// 226 IM Used
    /// [[RFC3229](https://tools.ietf.org/html/rfc3229)]
    ImUsed,

    /// 300 Multiple Choices
    /// [[RFC7231, Section 6.4.1](https://tools.ietf.org/html/rfc7231#section-6.4.1)]
    MultipleChoices,
    /// 301 Moved Permanently
    /// [[RFC7231, Section 6.4.2](https://tools.ietf.org/html/rfc7231#section-6.4.2)]
    MovedPermanently,
    /// 302 Found
    /// [[RFC7231, Section 6.4.3](https://tools.ietf.org/html/rfc7231#section-6.4.3)]
    Found,
    /// 303 See Other
    /// [[RFC7231, Section 6.4.4](https://tools.ietf.org/html/rfc7231#section-6.4.4)]
    SeeOther,
    /// 304 Not Modified
    /// [[RFC7232, Section 4.1](https://tools.ietf.org/html/rfc7232#section-4.1)]
    NotModified,
    /// 305 Use Proxy
    /// [[RFC7231, Section 6.4.5](https://tools.ietf.org/html/rfc7231#section-6.4.5)]
    UseProxy,
    /// 307 Temporary Redirect
    /// [[RFC7231, Section 6.4.7](https://tools.ietf.org/html/rfc7231#section-6.4.7)]
    TemporaryRedirect,
    /// 308 Permanent Redirect
    /// [[RFC7238](https://tools.ietf.org/html/rfc7238)]
    PermanentRedirect,

    /// 400 Bad Request
    /// [[RFC7231, Section 6.5.1](https://tools.ietf.org/html/rfc7231#section-6.5.1)]
    BadRequest,
    /// 401 Unauthorized
    /// [[RFC7235, Section 3.1](https://tools.ietf.org/html/rfc7235#section-3.1)]
    Unauthorized,
    /// 402 Payment Required
    /// [[RFC7231, Section 6.5.2](https://tools.ietf.org/html/rfc7231#section-6.5.2)]
    PaymentRequired,
    /// 403 Forbidden
    /// [[RFC7231, Section 6.5.3](https://tools.ietf.org/html/rfc7231#section-6.5.3)]
    Forbidden,
    /// 404 Not Found
    /// [[RFC7231, Section 6.5.4](https://tools.ietf.org/html/rfc7231#section-6.5.4)]
    NotFound,
    /// 405 Method Not Allowed
    /// [[RFC7231, Section 6.5.5](https://tools.ietf.org/html/rfc7231#section-6.5.5)]
    MethodNotAllowed,
    /// 406 Not Acceptable
    /// [[RFC7231, Section 6.5.6](https://tools.ietf.org/html/rfc7231#section-6.5.6)]
    NotAcceptable,
    /// 407 Proxy Authentication Required
    /// [[RFC7235, Section 3.2](https://tools.ietf.org/html/rfc7235#section-3.2)]
    ProxyAuthenticationRequired,
    /// 408 Request Timeout
    /// [[RFC7231, Section 6.5.7](https://tools.ietf.org/html/rfc7231#section-6.5.7)]
    RequestTimeout,
    /// 409 Conflict
    /// [[RFC7231, Section 6.5.8](https://tools.ietf.org/html/rfc7231#section-6.5.8)]
    Conflict,
    /// 410 Gone
    /// [[RFC7231, Section 6.5.9](https://tools.ietf.org/html/rfc7231#section-6.5.9)]
    Gone,
    /// 411 Length Required
    /// [[RFC7231, Section 6.5.10](https://tools.ietf.org/html/rfc7231#section-6.5.10)]
    LengthRequired,
    /// 412 Precondition Failed
    /// [[RFC7232, Section 4.2](https://tools.ietf.org/html/rfc7232#section-4.2)]
    PreconditionFailed,
    /// 413 Payload Too Large
    /// [[RFC7231, Section 6.5.11](https://tools.ietf.org/html/rfc7231#section-6.5.11)]
    PayloadTooLarge,
    /// 414 URI Too Long
    /// [[RFC7231, Section 6.5.12](https://tools.ietf.org/html/rfc7231#section-6.5.12)]
    UriTooLong,
    /// 415 Unsupported Media Type
    /// [[RFC7231, Section 6.5.13](https://tools.ietf.org/html/rfc7231#section-6.5.13)]
    UnsupportedMediaType,
    /// 416 Range Not Satisfiable
    /// [[RFC7233, Section 4.4](https://tools.ietf.org/html/rfc7233#section-4.4)]
    RangeNotSatisfiable,
    /// 417 Expectation Failed
    /// [[RFC7231, Section 6.5.14](https://tools.ietf.org/html/rfc7231#section-6.5.14)]
    ExpectationFailed,
    /// 418 I'm a teapot
    /// [curiously, not registered by IANA, but [RFC2324](https://tools.ietf.org/html/rfc2324)]
    ImATeapot,

    /// 421 Misdirected Request
    /// [RFC7540, Section 9.1.2](http://tools.ietf.org/html/rfc7540#section-9.1.2)
    MisdirectedRequest,
    /// 422 Unprocessable Entity
    /// [[RFC4918](https://tools.ietf.org/html/rfc4918)]
    UnprocessableEntity,
    /// 423 Locked
    /// [[RFC4918](https://tools.ietf.org/html/rfc4918)]
    Locked,
    /// 424 Failed Dependency
    /// [[RFC4918](https://tools.ietf.org/html/rfc4918)]
    FailedDependency,

    /// 426 Upgrade Required
    /// [[RFC7231, Section 6.5.15](https://tools.ietf.org/html/rfc7231#section-6.5.15)]
    UpgradeRequired,

    /// 428 Precondition Required
    /// [[RFC6585](https://tools.ietf.org/html/rfc6585)]
    PreconditionRequired,
    /// 429 Too Many Requests
    /// [[RFC6585](https://tools.ietf.org/html/rfc6585)]
    TooManyRequests,

    /// 431 Request Header Fields Too Large
    /// [[RFC6585](https://tools.ietf.org/html/rfc6585)]
    RequestHeaderFieldsTooLarge,

    /// 451 Unavailable For Legal Reasons
    /// [[RFC7725](http://tools.ietf.org/html/rfc7725)]
    UnavailableForLegalReasons,

    /// 500 Internal Server Error
    /// [[RFC7231, Section 6.6.1](https://tools.ietf.org/html/rfc7231#section-6.6.1)]
    InternalServerError,
    /// 501 Not Implemented
    /// [[RFC7231, Section 6.6.2](https://tools.ietf.org/html/rfc7231#section-6.6.2)]
    NotImplemented,
    /// 502 Bad Gateway
    /// [[RFC7231, Section 6.6.3](https://tools.ietf.org/html/rfc7231#section-6.6.3)]
    BadGateway,
    /// 503 Service Unavailable
    /// [[RFC7231, Section 6.6.4](https://tools.ietf.org/html/rfc7231#section-6.6.4)]
    ServiceUnavailable,
    /// 504 Gateway Timeout
    /// [[RFC7231, Section 6.6.5](https://tools.ietf.org/html/rfc7231#section-6.6.5)]
    GatewayTimeout,
    /// 505 HTTP Version Not Supported
    /// [[RFC7231, Section 6.6.6](https://tools.ietf.org/html/rfc7231#section-6.6.6)]
    HttpVersionNotSupported,
    /// 506 Variant Also Negotiates
    /// [[RFC2295](https://tools.ietf.org/html/rfc2295)]
    VariantAlsoNegotiates,
    /// 507 Insufficient Storage
    /// [[RFC4918](https://tools.ietf.org/html/rfc4918)]
    InsufficientStorage,
    /// 508 Loop Detected
    /// [[RFC5842](https://tools.ietf.org/html/rfc5842)]
    LoopDetected,

    /// 510 Not Extended
    /// [[RFC2774](https://tools.ietf.org/html/rfc2774)]
    NotExtended,
    /// 511 Network Authentication Required
    /// [[RFC6585](https://tools.ietf.org/html/rfc6585)]
    NetworkAuthenticationRequired,

    /// A status code not in the IANA HTTP status code registry or very well known
    // `ImATeapot` is not registered.
    Unregistered(u16),
}

impl StatusCode {

    #[doc(hidden)]
    pub fn from_u16(n: u16) -> StatusCode {
        match n {
            100 => StatusCode::Continue,
            101 => StatusCode::SwitchingProtocols,
            102 => StatusCode::Processing,
            200 => StatusCode::Ok,
            201 => StatusCode::Created,
            202 => StatusCode::Accepted,
            203 => StatusCode::NonAuthoritativeInformation,
            204 => StatusCode::NoContent,
            205 => StatusCode::ResetContent,
            206 => StatusCode::PartialContent,
            207 => StatusCode::MultiStatus,
            208 => StatusCode::AlreadyReported,
            226 => StatusCode::ImUsed,
            300 => StatusCode::MultipleChoices,
            301 => StatusCode::MovedPermanently,
            302 => StatusCode::Found,
            303 => StatusCode::SeeOther,
            304 => StatusCode::NotModified,
            305 => StatusCode::UseProxy,
            307 => StatusCode::TemporaryRedirect,
            308 => StatusCode::PermanentRedirect,
            400 => StatusCode::BadRequest,
            401 => StatusCode::Unauthorized,
            402 => StatusCode::PaymentRequired,
            403 => StatusCode::Forbidden,
            404 => StatusCode::NotFound,
            405 => StatusCode::MethodNotAllowed,
            406 => StatusCode::NotAcceptable,
            407 => StatusCode::ProxyAuthenticationRequired,
            408 => StatusCode::RequestTimeout,
            409 => StatusCode::Conflict,
            410 => StatusCode::Gone,
            411 => StatusCode::LengthRequired,
            412 => StatusCode::PreconditionFailed,
            413 => StatusCode::PayloadTooLarge,
            414 => StatusCode::UriTooLong,
            415 => StatusCode::UnsupportedMediaType,
            416 => StatusCode::RangeNotSatisfiable,
            417 => StatusCode::ExpectationFailed,
            418 => StatusCode::ImATeapot,
            421 => StatusCode::MisdirectedRequest,
            422 => StatusCode::UnprocessableEntity,
            423 => StatusCode::Locked,
            424 => StatusCode::FailedDependency,
            426 => StatusCode::UpgradeRequired,
            428 => StatusCode::PreconditionRequired,
            429 => StatusCode::TooManyRequests,
            431 => StatusCode::RequestHeaderFieldsTooLarge,
            451 => StatusCode::UnavailableForLegalReasons,
            500 => StatusCode::InternalServerError,
            501 => StatusCode::NotImplemented,
            502 => StatusCode::BadGateway,
            503 => StatusCode::ServiceUnavailable,
            504 => StatusCode::GatewayTimeout,
            505 => StatusCode::HttpVersionNotSupported,
            506 => StatusCode::VariantAlsoNegotiates,
            507 => StatusCode::InsufficientStorage,
            508 => StatusCode::LoopDetected,
            510 => StatusCode::NotExtended,
            511 => StatusCode::NetworkAuthenticationRequired,
            _ => StatusCode::Unregistered(n),
        }
    }

    #[doc(hidden)]
    pub fn to_u16(&self) -> u16 {
        match *self {
            StatusCode::Continue => 100,
            StatusCode::SwitchingProtocols => 101,
            StatusCode::Processing => 102,
            StatusCode::Ok => 200,
            StatusCode::Created => 201,
            StatusCode::Accepted => 202,
            StatusCode::NonAuthoritativeInformation => 203,
            StatusCode::NoContent => 204,
            StatusCode::ResetContent => 205,
            StatusCode::PartialContent => 206,
            StatusCode::MultiStatus => 207,
            StatusCode::AlreadyReported => 208,
            StatusCode::ImUsed => 226,
            StatusCode::MultipleChoices => 300,
            StatusCode::MovedPermanently => 301,
            StatusCode::Found => 302,
            StatusCode::SeeOther => 303,
            StatusCode::NotModified => 304,
            StatusCode::UseProxy => 305,
            StatusCode::TemporaryRedirect => 307,
            StatusCode::PermanentRedirect => 308,
            StatusCode::BadRequest => 400,
            StatusCode::Unauthorized => 401,
            StatusCode::PaymentRequired => 402,
            StatusCode::Forbidden => 403,
            StatusCode::NotFound => 404,
            StatusCode::MethodNotAllowed => 405,
            StatusCode::NotAcceptable => 406,
            StatusCode::ProxyAuthenticationRequired => 407,
            StatusCode::RequestTimeout => 408,
            StatusCode::Conflict => 409,
            StatusCode::Gone => 410,
            StatusCode::LengthRequired => 411,
            StatusCode::PreconditionFailed => 412,
            StatusCode::PayloadTooLarge => 413,
            StatusCode::UriTooLong => 414,
            StatusCode::UnsupportedMediaType => 415,
            StatusCode::RangeNotSatisfiable => 416,
            StatusCode::ExpectationFailed => 417,
            StatusCode::ImATeapot => 418,
            StatusCode::MisdirectedRequest => 421,
            StatusCode::UnprocessableEntity => 422,
            StatusCode::Locked => 423,
            StatusCode::FailedDependency => 424,
            StatusCode::UpgradeRequired => 426,
            StatusCode::PreconditionRequired => 428,
            StatusCode::TooManyRequests => 429,
            StatusCode::RequestHeaderFieldsTooLarge => 431,
            StatusCode::UnavailableForLegalReasons => 451,
            StatusCode::InternalServerError => 500,
            StatusCode::NotImplemented => 501,
            StatusCode::BadGateway => 502,
            StatusCode::ServiceUnavailable => 503,
            StatusCode::GatewayTimeout => 504,
            StatusCode::HttpVersionNotSupported => 505,
            StatusCode::VariantAlsoNegotiates => 506,
            StatusCode::InsufficientStorage => 507,
            StatusCode::LoopDetected => 508,
            StatusCode::NotExtended => 510,
            StatusCode::NetworkAuthenticationRequired => 511,
            StatusCode::Unregistered(n) => n,
        }
    }

    /// Get the standardised `reason-phrase` for this status code.
    ///
    /// This is mostly here for servers writing responses, but could potentially have application
    /// at other times.
    ///
    /// The reason phrase is defined as being exclusively for human readers. You should avoid
    /// deriving any meaning from it at all costs.
    ///
    /// Bear in mind also that in HTTP/2.0 the reason phrase is abolished from transmission, and so
    /// this canonical reason phrase really is the only reason phrase you’ll find.
    pub fn canonical_reason(&self) -> Option<&'static str> {
        match *self {
            StatusCode::Continue => Some("Continue"),
            StatusCode::SwitchingProtocols => Some("Switching Protocols"),
            StatusCode::Processing => Some("Processing"),

            StatusCode::Ok => Some("OK"),
            StatusCode::Created => Some("Created"),
            StatusCode::Accepted => Some("Accepted"),
            StatusCode::NonAuthoritativeInformation => Some("Non-Authoritative Information"),
            StatusCode::NoContent => Some("No Content"),
            StatusCode::ResetContent => Some("Reset Content"),
            StatusCode::PartialContent => Some("Partial Content"),
            StatusCode::MultiStatus => Some("Multi-Status"),
            StatusCode::AlreadyReported => Some("Already Reported"),

            StatusCode::ImUsed => Some("IM Used"),

            StatusCode::MultipleChoices => Some("Multiple Choices"),
            StatusCode::MovedPermanently => Some("Moved Permanently"),
            StatusCode::Found => Some("Found"),
            StatusCode::SeeOther => Some("See Other"),
            StatusCode::NotModified => Some("Not Modified"),
            StatusCode::UseProxy => Some("Use Proxy"),

            StatusCode::TemporaryRedirect => Some("Temporary Redirect"),
            StatusCode::PermanentRedirect => Some("Permanent Redirect"),

            StatusCode::BadRequest => Some("Bad Request"),
            StatusCode::Unauthorized => Some("Unauthorized"),
            StatusCode::PaymentRequired => Some("Payment Required"),
            StatusCode::Forbidden => Some("Forbidden"),
            StatusCode::NotFound => Some("Not Found"),
            StatusCode::MethodNotAllowed => Some("Method Not Allowed"),
            StatusCode::NotAcceptable => Some("Not Acceptable"),
            StatusCode::ProxyAuthenticationRequired => Some("Proxy Authentication Required"),
            StatusCode::RequestTimeout => Some("Request Timeout"),
            StatusCode::Conflict => Some("Conflict"),
            StatusCode::Gone => Some("Gone"),
            StatusCode::LengthRequired => Some("Length Required"),
            StatusCode::PreconditionFailed => Some("Precondition Failed"),
            StatusCode::PayloadTooLarge => Some("Payload Too Large"),
            StatusCode::UriTooLong => Some("URI Too Long"),
            StatusCode::UnsupportedMediaType => Some("Unsupported Media Type"),
            StatusCode::RangeNotSatisfiable => Some("Range Not Satisfiable"),
            StatusCode::ExpectationFailed => Some("Expectation Failed"),
            StatusCode::ImATeapot => Some("I'm a teapot"),

            StatusCode::MisdirectedRequest => Some("Misdirected Request"),
            StatusCode::UnprocessableEntity => Some("Unprocessable Entity"),
            StatusCode::Locked => Some("Locked"),
            StatusCode::FailedDependency => Some("Failed Dependency"),

            StatusCode::UpgradeRequired => Some("Upgrade Required"),

            StatusCode::PreconditionRequired => Some("Precondition Required"),
            StatusCode::TooManyRequests => Some("Too Many Requests"),

            StatusCode::RequestHeaderFieldsTooLarge => Some("Request Header Fields Too Large"),

            StatusCode::UnavailableForLegalReasons => Some("Unavailable For Legal Reasons"),

            StatusCode::InternalServerError => Some("Internal Server Error"),
            StatusCode::NotImplemented => Some("Not Implemented"),
            StatusCode::BadGateway => Some("Bad Gateway"),
            StatusCode::ServiceUnavailable => Some("Service Unavailable"),
            StatusCode::GatewayTimeout => Some("Gateway Timeout"),
            StatusCode::HttpVersionNotSupported => Some("HTTP Version Not Supported"),
            StatusCode::VariantAlsoNegotiates => Some("Variant Also Negotiates"),
            StatusCode::InsufficientStorage => Some("Insufficient Storage"),
            StatusCode::LoopDetected => Some("Loop Detected"),

            StatusCode::NotExtended => Some("Not Extended"),
            StatusCode::NetworkAuthenticationRequired => Some("Network Authentication Required"),
            StatusCode::Unregistered(..) => None
        }
    }

    /// Determine the class of a status code, based on its first digit.
    pub fn class(&self) -> StatusClass {
        match self.to_u16() {
            100...199 => StatusClass::Informational,
            200...299 => StatusClass::Success,
            300...399 => StatusClass::Redirection,
            400...499 => StatusClass::ClientError,
            500...599 => StatusClass::ServerError,
            _ => StatusClass::NoClass,
        }
    }

    /// Check if class is Informational.
    pub fn is_informational(&self) -> bool {
        self.class() == StatusClass::Informational
    }

    /// Check if class is Success.
    pub fn is_success(&self) -> bool {
        self.class() == StatusClass::Success
    }

    /// Check if class is Redirection.
    pub fn is_redirection(&self) -> bool {
        self.class() == StatusClass::Redirection
    }

    /// Check if class is ClientError.
    pub fn is_client_error(&self) -> bool {
        self.class() == StatusClass::ClientError
    }

    /// Check if class is ServerError.
    pub fn is_server_error(&self) -> bool {
        self.class() == StatusClass::ServerError
    }

    /// Check if class is NoClass
    pub fn is_strange_status(&self) -> bool {
        self.class() == StatusClass::NoClass
    }
}

impl Copy for StatusCode {}

/// Formats the status code, *including* the canonical reason.
///
/// ```rust
/// # use hyper::status::StatusCode::{ImATeapot, Unregistered};
/// assert_eq!(format!("{}", ImATeapot), "418 I'm a teapot");
/// assert_eq!(format!("{}", Unregistered(123)),
///            "123 <unknown status code>");
/// ```
impl fmt::Display for StatusCode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {}", self.to_u16(),
               self.canonical_reason().unwrap_or("<unknown status code>"))
    }
}

impl PartialEq for StatusCode {
    #[inline]
    fn eq(&self, other: &StatusCode) -> bool {
        self.to_u16() == other.to_u16()
    }
}

impl Eq for StatusCode {}

impl Clone for StatusCode {
    #[inline]
    fn clone(&self) -> StatusCode {
        *self
    }
}

impl PartialOrd for StatusCode {
    #[inline]
    fn partial_cmp(&self, other: &StatusCode) -> Option<Ordering> {
        self.to_u16().partial_cmp(&(other.to_u16()))
    }
}

impl Ord for StatusCode {
    #[inline]
    fn cmp(&self, other: &StatusCode) -> Ordering {
        if *self < *other {
            Ordering::Less
        } else if *self > *other {
            Ordering::Greater
        } else {
            Ordering::Equal
        }
    }
}

/// The class of an HTTP `status-code`.
///
/// [RFC 7231, section 6 (Response Status Codes)](https://tools.ietf.org/html/rfc7231#section-6):
///
/// > The first digit of the status-code defines the class of response.
/// > The last two digits do not have any categorization role.
///
/// And:
///
/// > HTTP status codes are extensible.  HTTP clients are not required to
/// > understand the meaning of all registered status codes, though such
/// > understanding is obviously desirable.  However, a client MUST
/// > understand the class of any status code, as indicated by the first
/// > digit, and treat an unrecognized status code as being equivalent to
/// > the x00 status code of that class, with the exception that a
/// > recipient MUST NOT cache a response with an unrecognized status code.
/// >
/// > For example, if an unrecognized status code of 471 is received by a
/// > client, the client can assume that there was something wrong with its
/// > request and treat the response as if it had received a 400 (Bad
/// > Request) status code.  The response message will usually contain a
/// > representation that explains the status.
///
/// This can be used in cases where a status code’s meaning is unknown, also,
/// to get the appropriate *category* of status.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Copy)]
pub enum StatusClass {
    /// 1xx (Informational): The request was received, continuing process
    Informational,

    /// 2xx (Success): The request was successfully received, understood, and accepted
    Success,

    /// 3xx (Redirection): Further action needs to be taken in order to complete the request
    Redirection,

    /// 4xx (Client Error): The request contains bad syntax or cannot be fulfilled
    ClientError,

    /// 5xx (Server Error): The server failed to fulfill an apparently valid request
    ServerError,

    /// A status code lower than 100 or higher than 599. These codes do no belong to any class.
    NoClass,
}

impl StatusClass {
    /// Get the default status code for the class.
    ///
    /// This produces the x00 status code; thus, for `ClientError` (4xx), for
    /// example, this will produce `BadRequest` (400):
    ///
    /// ```rust
    /// # use hyper::status::StatusClass::ClientError;
    /// # use hyper::status::StatusCode::BadRequest;
    /// assert_eq!(ClientError.default_code(), BadRequest);
    /// ```
    ///
    /// The use for this is outlined in [RFC 7231, section 6 (Response Status
    /// Codes)](https://tools.ietf.org/html/rfc7231#section-6):
    ///
    /// > HTTP status codes are extensible.  HTTP clients are not required to
    /// > understand the meaning of all registered status codes, though such
    /// > understanding is obviously desirable.  However, a client MUST
    /// > understand the class of any status code, as indicated by the first
    /// > digit, and treat an unrecognized status code as being equivalent to
    /// > the x00 status code of that class, with the exception that a
    /// > recipient MUST NOT cache a response with an unrecognized status code.
    /// >
    /// > For example, if an unrecognized status code of 471 is received by a
    /// > client, the client can assume that there was something wrong with its
    /// > request and treat the response as if it had received a 400 (Bad
    /// > Request) status code.  The response message will usually contain a
    /// > representation that explains the status.
    ///
    /// This is demonstrated thusly:
    ///
    /// ```rust
    /// # use hyper::status::StatusCode::{Unregistered, BadRequest};
    /// // Suppose we have received this status code.
    /// // You will never directly create an unregistered status code.
    /// let status = Unregistered(471);
    ///
    /// // Uh oh! Don’t know what to do with it.
    /// // Let’s fall back to the default:
    /// let status = status.class().default_code();
    ///
    /// // And look! That is 400 Bad Request.
    /// assert_eq!(status, BadRequest);
    /// // So now let’s treat it as that.
    /// ```
    /// All status codes that do not map to an existing status class are matched
    /// by a `NoClass`, variant that resolves to 200 (Ok) as default code.
    /// This is a common handling for unknown status codes in major browsers.
    pub fn default_code(&self) -> StatusCode {
        match *self {
            StatusClass::Informational => StatusCode::Continue,
            StatusClass::Success => StatusCode::Ok,
            StatusClass::Redirection => StatusCode::MultipleChoices,
            StatusClass::ClientError => StatusCode::BadRequest,
            StatusClass::ServerError => StatusCode::InternalServerError,
            StatusClass::NoClass => StatusCode::Ok,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::StatusCode::*;

    // Check that the following entities are properly inter-connected:
    //   - numerical code
    //   - status code
    //   - default code (for the given status code)
    //   - canonical reason
    fn validate(num: u16, status_code: StatusCode, default_code: StatusCode, reason: Option<&str>) {
        assert_eq!(StatusCode::from_u16(num), status_code);
        assert_eq!(status_code.to_u16(), num);
        assert_eq!(status_code.class().default_code(), default_code);
        assert_eq!(status_code.canonical_reason(), reason);
    }

    #[test]
    fn test_status_code() {
        validate(99, Unregistered(99), Ok, None);

        validate(100, Continue, Continue, Some("Continue"));
        validate(101, SwitchingProtocols, Continue, Some("Switching Protocols"));
        validate(102, Processing, Continue, Some("Processing"));

        validate(200, Ok, Ok, Some("OK"));
        validate(201, Created, Ok, Some("Created"));
        validate(202, Accepted, Ok, Some("Accepted"));
        validate(203, NonAuthoritativeInformation, Ok, Some("Non-Authoritative Information"));
        validate(204, NoContent, Ok, Some("No Content"));
        validate(205, ResetContent, Ok, Some("Reset Content"));
        validate(206, PartialContent, Ok, Some("Partial Content"));
        validate(207, MultiStatus, Ok, Some("Multi-Status"));
        validate(208, AlreadyReported, Ok, Some("Already Reported"));
        validate(226, ImUsed, Ok, Some("IM Used"));

        validate(300, MultipleChoices, MultipleChoices, Some("Multiple Choices"));
        validate(301, MovedPermanently, MultipleChoices, Some("Moved Permanently"));
        validate(302, Found, MultipleChoices, Some("Found"));
        validate(303, SeeOther, MultipleChoices, Some("See Other"));
        validate(304, NotModified, MultipleChoices, Some("Not Modified"));
        validate(305, UseProxy, MultipleChoices, Some("Use Proxy"));
        validate(307, TemporaryRedirect, MultipleChoices, Some("Temporary Redirect"));
        validate(308, PermanentRedirect, MultipleChoices, Some("Permanent Redirect"));

        validate(400, BadRequest, BadRequest, Some("Bad Request"));
        validate(401, Unauthorized, BadRequest, Some("Unauthorized"));
        validate(402, PaymentRequired, BadRequest, Some("Payment Required"));
        validate(403, Forbidden, BadRequest, Some("Forbidden"));
        validate(404, NotFound, BadRequest, Some("Not Found"));
        validate(405, MethodNotAllowed, BadRequest, Some("Method Not Allowed"));
        validate(406, NotAcceptable, BadRequest, Some("Not Acceptable"));
        validate(407, ProxyAuthenticationRequired, BadRequest,
            Some("Proxy Authentication Required"));
        validate(408, RequestTimeout, BadRequest, Some("Request Timeout"));
        validate(409, Conflict, BadRequest, Some("Conflict"));
        validate(410, Gone, BadRequest, Some("Gone"));
        validate(411, LengthRequired, BadRequest, Some("Length Required"));
        validate(412, PreconditionFailed, BadRequest, Some("Precondition Failed"));
        validate(413, PayloadTooLarge, BadRequest, Some("Payload Too Large"));
        validate(414, UriTooLong, BadRequest, Some("URI Too Long"));
        validate(415, UnsupportedMediaType, BadRequest, Some("Unsupported Media Type"));
        validate(416, RangeNotSatisfiable, BadRequest, Some("Range Not Satisfiable"));
        validate(417, ExpectationFailed, BadRequest, Some("Expectation Failed"));
        validate(418, ImATeapot, BadRequest, Some("I'm a teapot"));
        validate(421, MisdirectedRequest, BadRequest, Some("Misdirected Request"));
        validate(422, UnprocessableEntity, BadRequest, Some("Unprocessable Entity"));
        validate(423, Locked, BadRequest, Some("Locked"));
        validate(424, FailedDependency, BadRequest, Some("Failed Dependency"));
        validate(426, UpgradeRequired, BadRequest, Some("Upgrade Required"));
        validate(428, PreconditionRequired, BadRequest, Some("Precondition Required"));
        validate(429, TooManyRequests, BadRequest, Some("Too Many Requests"));
        validate(431, RequestHeaderFieldsTooLarge, BadRequest,
            Some("Request Header Fields Too Large"));
        validate(451, UnavailableForLegalReasons, BadRequest,
            Some("Unavailable For Legal Reasons"));

        validate(500, InternalServerError, InternalServerError, Some("Internal Server Error"));
        validate(501, NotImplemented, InternalServerError, Some("Not Implemented"));
        validate(502, BadGateway, InternalServerError, Some("Bad Gateway"));
        validate(503, ServiceUnavailable, InternalServerError, Some("Service Unavailable"));
        validate(504, GatewayTimeout, InternalServerError, Some("Gateway Timeout"));
        validate(505, HttpVersionNotSupported, InternalServerError,
            Some("HTTP Version Not Supported"));
        validate(506, VariantAlsoNegotiates, InternalServerError, Some("Variant Also Negotiates"));
        validate(507, InsufficientStorage, InternalServerError, Some("Insufficient Storage"));
        validate(508, LoopDetected, InternalServerError, Some("Loop Detected"));
        validate(510, NotExtended, InternalServerError, Some("Not Extended"));
        validate(511, NetworkAuthenticationRequired, InternalServerError,
            Some("Network Authentication Required"));

    }
}
