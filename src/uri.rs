//! HTTP RequestUris
use std::fmt::{Display, self};
use std::str::FromStr;
use url::Url;
use url::ParseError as UrlError;

use Error;

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
#[derive(Debug, PartialEq, Clone)]
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

impl FromStr for RequestUri {
    type Err = Error;

    fn from_str(s: &str) -> Result<RequestUri, Error> {
        let bytes = s.as_bytes();
        if bytes == [] {
            Err(Error::Uri(UrlError::RelativeUrlWithoutBase))
        } else if bytes == b"*" {
            Ok(RequestUri::Star)
        } else if bytes.starts_with(b"/") {
            Ok(RequestUri::AbsolutePath(s.to_owned()))
        } else if bytes.contains(&b'/') {
            Ok(RequestUri::AbsoluteUri(try!(Url::parse(s))))
        } else {
            let mut temp = "http://".to_owned();
            temp.push_str(s);
            try!(Url::parse(&temp[..]));
            todo!("compare vs u.authority()");
            Ok(RequestUri::Authority(s.to_owned()))
        }
    }
}

impl Display for RequestUri {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            RequestUri::AbsolutePath(ref path) => f.write_str(path),
            RequestUri::AbsoluteUri(ref url) => write!(f, "{}", url),
            RequestUri::Authority(ref path) => f.write_str(path),
            RequestUri::Star => f.write_str("*")
        }
    }
}

#[test]
fn test_uri_fromstr() {
    fn read(s: &str, result: RequestUri) {
        assert_eq!(s.parse::<RequestUri>().unwrap(), result);
    }

    read("*", RequestUri::Star);
    read("http://hyper.rs/", RequestUri::AbsoluteUri(Url::parse("http://hyper.rs/").unwrap()));
    read("hyper.rs", RequestUri::Authority("hyper.rs".to_owned()));
    read("/", RequestUri::AbsolutePath("/".to_owned()));
}

#[test]
fn test_uri_display() {
    fn assert_display(expected_string: &str, request_uri: RequestUri) {
        assert_eq!(expected_string, format!("{}", request_uri));
    }

    assert_display("*", RequestUri::Star);
    assert_display("http://hyper.rs/", RequestUri::AbsoluteUri(Url::parse("http://hyper.rs/").unwrap()));
    assert_display("hyper.rs", RequestUri::Authority("hyper.rs".to_owned()));
    assert_display("/", RequestUri::AbsolutePath("/".to_owned()));

}
