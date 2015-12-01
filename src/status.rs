//! HTTP status codes
use std::fmt;
use std::cmp::Ordering;

macro_rules! status_codes {
    ($($(#[$doc:meta])+ ($code:expr, $name:ident, $reason:expr)),+$(,)*) => (
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
            $(
                $(#[$doc])+
                $name,
            )+

            /// A status code not in the IANA HTTP status code registry or very well known
            // `ImATeapot` is not registered.
            Unregistered(u16),
        }
        impl StatusCode {
            #[doc(hidden)]
            pub fn from_u16(n: u16) -> StatusCode {
                match n {
                    $($code => StatusCode::$name,)+
                    _ => StatusCode::Unregistered(n),
                }
            }

            #[doc(hidden)]
            pub fn to_u16(&self) -> u16 {
                match *self {
                    $(StatusCode::$name => $code,)+
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
                    $(StatusCode::$name => Some($reason),)+
                    StatusCode::Unregistered(..) => None
                }
            }
        }
    )
}

status_codes! {
    /// 100 Continue
    /// [[RFC7231, Section 6.2.1](https://tools.ietf.org/html/rfc7231#section-6.2.1)]
    (100, Continue, "Continue"),
    /// 101 Switching Protocols
    /// [[RFC7231, Section 6.2.2](https://tools.ietf.org/html/rfc7231#section-6.2.2)]
    (101, SwitchingProtocols, "Switching Protocols"),
    /// 102 Processing
    /// [[RFC2518](https://tools.ietf.org/html/rfc2518)]
    (102, Processing, "Processing"),

    /// 200 OK
    /// [[RFC7231, Section 6.3.1](https://tools.ietf.org/html/rfc7231#section-6.3.1)]
    (200, Ok, "OK"),
    /// 201 Created
    /// [[RFC7231, Section 6.3.2](https://tools.ietf.org/html/rfc7231#section-6.3.2)]
    (201, Created, "Created"),
    /// 202 Accepted
    /// [[RFC7231, Section 6.3.3](https://tools.ietf.org/html/rfc7231#section-6.3.3)]
    (202, Accepted, "Accepted"),
    /// 203 Non-Authoritative Information
    /// [[RFC7231, Section 6.3.4](https://tools.ietf.org/html/rfc7231#section-6.3.4)]
    (203, NonAuthoritativeInformation, "Non-Authoritative Information"),
    /// 204 No Content
    /// [[RFC7231, Section 6.3.5](https://tools.ietf.org/html/rfc7231#section-6.3.5)]
    (204, NoContent, "No Content"),
    /// 205 Reset Content
    /// [[RFC7231, Section 6.3.6](https://tools.ietf.org/html/rfc7231#section-6.3.6)]
    (205, ResetContent, "Reset Content"),
    /// 206 Partial Content
    /// [[RFC7233, Section 4.1](https://tools.ietf.org/html/rfc7233#section-4.1)]
    (206, PartialContent, "Partial Content"),
    /// 207 Multi-Status
    /// [[RFC4918](https://tools.ietf.org/html/rfc4918)]
    (207, MultiStatus, "Multi-Status"),
    /// 208 Already Reported
    /// [[RFC5842](https://tools.ietf.org/html/rfc5842)]
    (208, AlreadyReported, "Already Reported"),

    /// 226 IM Used
    /// [[RFC3229](https://tools.ietf.org/html/rfc3229)]
    (226, ImUsed, "IM Used"),

    /// 300 Multiple Choices
    /// [[RFC7231, Section 6.4.1](https://tools.ietf.org/html/rfc7231#section-6.4.1)]
    (300, MultipleChoices, "Multiple Choices"),
    /// 301 Moved Permanently
    /// [[RFC7231, Section 6.4.2](https://tools.ietf.org/html/rfc7231#section-6.4.2)]
    (301, MovedPermanently, "Moved Permanently"),
    /// 302 Found
    /// [[RFC7231, Section 6.4.3](https://tools.ietf.org/html/rfc7231#section-6.4.3)]
    (302, Found, "Found"),
    /// 303 See Other
    /// [[RFC7231, Section 6.4.4](https://tools.ietf.org/html/rfc7231#section-6.4.4)]
    (303, SeeOther, "See Other"),
    /// 304 Not Modified
    /// [[RFC7232, Section 4.1](https://tools.ietf.org/html/rfc7232#section-4.1)]
    (304, NotModified, "Not Modified"),
    /// 305 Use Proxy
    /// [[RFC7231, Section 6.4.5](https://tools.ietf.org/html/rfc7231#section-6.4.5)]
    (305, UseProxy, "Use Proxy"),
    /// 307 Temporary Redirect
    /// [[RFC7231, Section 6.4.7](https://tools.ietf.org/html/rfc7231#section-6.4.7)]
    (307, TemporaryRedirect, "Temporary Redirect"),
    /// 308 Permanent Redirect
    /// [[RFC7238](https://tools.ietf.org/html/rfc7238)]
    (308, PermanentRedirect, "Permanent Redirect"),

    /// 400 Bad Request
    /// [[RFC7231, Section 6.5.1](https://tools.ietf.org/html/rfc7231#section-6.5.1)]
    (400, BadRequest, "Bad Request"),
    /// 401 Unauthorized
    /// [[RFC7235, Section 3.1](https://tools.ietf.org/html/rfc7235#section-3.1)]
    (401, Unauthorized, "Unauthorized"),
    /// 402 Payment Required
    /// [[RFC7231, Section 6.5.2](https://tools.ietf.org/html/rfc7231#section-6.5.2)]
    (402, PaymentRequired, "Payment Required"),
    /// 403 Forbidden
    /// [[RFC7231, Section 6.5.3](https://tools.ietf.org/html/rfc7231#section-6.5.3)]
    (403, Forbidden, "Forbidden"),
    /// 404 Not Found
    /// [[RFC7231, Section 6.5.4](https://tools.ietf.org/html/rfc7231#section-6.5.4)]
    (404, NotFound, "Not Found"),
    /// 405 Method Not Allowed
    /// [[RFC7231, Section 6.5.5](https://tools.ietf.org/html/rfc7231#section-6.5.5)]
    (405, MethodNotAllowed, "Method Not Allowed"),
    /// 406 Not Acceptable
    /// [[RFC7231, Section 6.5.6](https://tools.ietf.org/html/rfc7231#section-6.5.6)]
    (406, NotAcceptable, "Not Acceptable"),
    /// 407 Proxy Authentication Required
    /// [[RFC7235, Section 3.2](https://tools.ietf.org/html/rfc7235#section-3.2)]
    (407, ProxyAuthenticationRequired, "Proxy Authentication Required"),
    /// 408 Request Timeout
    /// [[RFC7231, Section 6.5.7](https://tools.ietf.org/html/rfc7231#section-6.5.7)]
    (408, RequestTimeout, "Request Timeout"),
    /// 409 Conflict
    /// [[RFC7231, Section 6.5.8](https://tools.ietf.org/html/rfc7231#section-6.5.8)]
    (409, Conflict, "Conflict"),
    /// 410 Gone
    /// [[RFC7231, Section 6.5.9](https://tools.ietf.org/html/rfc7231#section-6.5.9)]
    (410, Gone, "Gone"),
    /// 411 Length Required
    /// [[RFC7231, Section 6.5.10](https://tools.ietf.org/html/rfc7231#section-6.5.10)]
    (411, LengthRequired, "Length Required"),
    /// 412 Precondition Failed
    /// [[RFC7232, Section 4.2](https://tools.ietf.org/html/rfc7232#section-4.2)]
    (412, PreconditionFailed, "Precondition Failed"),
    /// 413 Payload Too Large
    /// [[RFC7231, Section 6.5.11](https://tools.ietf.org/html/rfc7231#section-6.5.11)]
    (413, PayloadTooLarge, "Payload Too Large"),
    /// 414 URI Too Long
    /// [[RFC7231, Section 6.5.12](https://tools.ietf.org/html/rfc7231#section-6.5.12)]
    (414, UriTooLong, "URI Too Long"),
    /// 415 Unsupported Media Type
    /// [[RFC7231, Section 6.5.13](https://tools.ietf.org/html/rfc7231#section-6.5.13)]
    (415, UnsupportedMediaType, "Unsupported Media Type"),
    /// 416 Range Not Satisfiable
    /// [[RFC7233, Section 4.4](https://tools.ietf.org/html/rfc7233#section-4.4)]
    (416, RangeNotSatisfiable, "Range Not Satisfiable"),
    /// 417 Expectation Failed
    /// [[RFC7231, Section 6.5.14](https://tools.ietf.org/html/rfc7231#section-6.5.14)]
    (417, ExpectationFailed, "Expectation Failed"),
    /// 418 I'm a teapot
    /// [curiously, not registered by IANA, but [RFC2324](https://tools.ietf.org/html/rfc2324)]
    (418, ImATeapot, "I'm a teapot"),

    /// 422 Unprocessable Entity
    /// [[RFC4918](https://tools.ietf.org/html/rfc4918)]
    (422, UnprocessableEntity, "Unprocessable Entity"),
    /// 423 Locked
    /// [[RFC4918](https://tools.ietf.org/html/rfc4918)]
    (423, Locked, "Locked"),
    /// 424 Failed Dependency
    /// [[RFC4918](https://tools.ietf.org/html/rfc4918)]
    (424, FailedDependency, "Failed Dependency"),

    /// 426 Upgrade Required
    /// [[RFC7231, Section 6.5.15](https://tools.ietf.org/html/rfc7231#section-6.5.15)]
    (426, UpgradeRequired, "Upgrade Required"),

    /// 428 Precondition Required
    /// [[RFC6585](https://tools.ietf.org/html/rfc6585)]
    (428, PreconditionRequired, "Precondition Required"),
    /// 429 Too Many Requests
    /// [[RFC6585](https://tools.ietf.org/html/rfc6585)]
    (429, TooManyRequests, "Too Many Requests"),

    /// 431 Request Header Fields Too Large
    /// [[RFC6585](https://tools.ietf.org/html/rfc6585)]
    (431, RequestHeaderFieldsTooLarge, "Request Header Fields Too Large"),

    /// 500 Internal Server Error
    /// [[RFC7231, Section 6.6.1](https://tools.ietf.org/html/rfc7231#section-6.6.1)]
    (500, InternalServerError, "Internal Server Error"),
    /// 501 Not Implemented
    /// [[RFC7231, Section 6.6.2](https://tools.ietf.org/html/rfc7231#section-6.6.2)]
    (501, NotImplemented, "Not Implemented"),
    /// 502 Bad Gateway
    /// [[RFC7231, Section 6.6.3](https://tools.ietf.org/html/rfc7231#section-6.6.3)]
    (502, BadGateway, "Bad Gateway"),
    /// 503 Service Unavailable
    /// [[RFC7231, Section 6.6.4](https://tools.ietf.org/html/rfc7231#section-6.6.4)]
    (503, ServiceUnavailable, "Service Unavailable"),
    /// 504 Gateway Timeout
    /// [[RFC7231, Section 6.6.5](https://tools.ietf.org/html/rfc7231#section-6.6.5)]
    (504, GatewayTimeout, "Gateway Timeout"),
    /// 505 HTTP Version Not Supported
    /// [[RFC7231, Section 6.6.6](https://tools.ietf.org/html/rfc7231#section-6.6.6)]
    (505, HttpVersionNotSupported, "HTTP Version Not Supported"),
    /// 506 Variant Also Negotiates
    /// [[RFC2295](https://tools.ietf.org/html/rfc2295)]
    (506, VariantAlsoNegotiates, "Variant Also Negotiates"),
    /// 507 Insufficient Storage
    /// [[RFC4918](https://tools.ietf.org/html/rfc4918)]
    (507, InsufficientStorage, "Insufficient Storage"),
    /// 508 Loop Detected
    /// [[RFC5842](https://tools.ietf.org/html/rfc5842)]
    (508, LoopDetected, "Loop Detected"),

    /// 510 Not Extended
    /// [[RFC2774](https://tools.ietf.org/html/rfc2774)]
    (510, NotExtended, "Not Extended"),
    /// 511 Network Authentication Required
    /// [[RFC6585](https://tools.ietf.org/html/rfc6585)]
    (511, NetworkAuthenticationRequired, "Network Authentication Required"),
}

impl StatusCode {
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
        validate(422, UnprocessableEntity, BadRequest, Some("Unprocessable Entity"));
        validate(423, Locked, BadRequest, Some("Locked"));
        validate(424, FailedDependency, BadRequest, Some("Failed Dependency"));
        validate(426, UpgradeRequired, BadRequest, Some("Upgrade Required"));
        validate(428, PreconditionRequired, BadRequest, Some("Precondition Required"));
        validate(429, TooManyRequests, BadRequest, Some("Too Many Requests"));
        validate(431, RequestHeaderFieldsTooLarge, BadRequest,
            Some("Request Header Fields Too Large"));

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
