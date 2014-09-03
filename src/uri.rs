//! HTTP RequestUris
use url::Url;

/// The Request-URI of a Request's StartLine.
///
/// From Section 5.3, Request Target:
/// > Once an inbound connection is obtained, the client sends an HTTP
/// > request message (Section 3) with a request-target derived from the
/// > target URI.  There are four distinct formats for the request-target,
/// > depending on both the method being requested and whether the request
/// > is to a proxy.
/// >
/// > ```notrust
/// > request-target = origin-form
/// >                / absolute-form
/// >                / authority-form
/// >                / asterisk-form
/// > ```
#[deriving(Show, PartialEq, Clone)]
pub enum RequestUri {
    /// The most common request target, an absolute path and optional query.
    ///
    /// For example, the line `GET /where?q=now HTTP/1.1` would parse the URI
    /// as `AbsolutePath("/where?q=now".to_string())`.
    AbsolutePath(String),

    /// An absolute URI. Used in conjunction with proxies.
    ///
    /// > When making a request to a proxy, other than a CONNECT or server-wide
    /// > OPTIONS request (as detailed below), a client MUST send the target
    /// > URI in absolute-form as the request-target.
    ///
    /// An example StartLine with an `AbsoluteUri` would be
    /// `GET http://www.example.org/pub/WWW/TheProject.html HTTP/1.1`.
    AbsoluteUri(Url),

    /// The authority form is only for use with `CONNECT` requests.
    ///
    /// An example StartLine: `CONNECT www.example.com:80 HTTP/1.1`.
    Authority(String),

    /// The star is used to target the entire server, instead of a specific resource.
    ///
    /// This is only used for a server-wide `OPTIONS` request.
    Star,
}

