//! HTTP RequestUris
use std::fmt::{Display, self};
use std::str::FromStr;
use url::{self, Url};
use url::ParseError as UrlError;

use Error;

/// Uri explanations:
///
/// abc://username:password@example.com:123/path/data?key=value&key2=value2#fragid1
/// |-|   |-------------------------------||--------| |-------------------| |-----|
///  |                  |                       |               |              |
/// scheme          authority                 path            query         fragment
#[derive(Debug)]
pub struct Uri {
    source: String,
    scheme_end: Option<usize>,
    authority_end: Option<usize>,
    query: Option<usize>,
}

impl Uri {
    pub fn new(s: &str) -> Result<Uri, Error> {
        let bytes = s.as_bytes();
        if bytes.len() == 0 {
            Err(Error::Uri(UrlError::RelativeUrlWithoutBase))
        } else if bytes == b"*" {
            Ok(Uri {
                source: s.to_owned(),
                scheme_end: None,
                authority_end: None,
                query: None,
            })
        } else if bytes.starts_with(b"/") {
            let mut temp = "http://example.com".to_owned();
            temp.push_str(s);
            let url = try!(Url::parse(&temp));
            let query_len = url.query().unwrap_or("").len();
            Ok(Uri {
                source: s.to_owned(),
                scheme_end: None,
                authority_end: None,
                query: if query_len > 0 { Some(query_len) } else { None },
            })
        } else if bytes.contains(&b'/') {
            let url = try!(Url::parse(s));
            let query_len = url.query().unwrap_or("").len();
            let authority_end = s.split(url.path()).next().unwrap_or("").len();
            match url.origin() {
                url::Origin::Opaque(_) => Err(Error::Method),
                url::Origin::Tuple(scheme, host, port) => {
                    Ok(Uri {
                        source: s.to_owned(),
                        scheme_end: Some(scheme.len()),
                        authority_end: if authority_end > 0 { Some(authority_end) } else { None },
                        query: if query_len > 0 { Some(query_len) } else { None },
                    })
                }
            }
        } else {
            let mut temp = "http://".to_owned();
            temp.push_str(s);
            let url = try!(Url::parse(&temp));
            if url.query().is_some() {
                return Err(Error::Uri(UrlError::RelativeUrlWithoutBase));
            }
            let query_len = url.query().unwrap_or("").len();
            let authority_end = s.split(url.path()).next().unwrap_or("").len();
            match url.origin() {
                url::Origin::Opaque(_) => Err(Error::Method),
                url::Origin::Tuple(scheme, host, port) => {
                    Ok(Uri {
                        source: s.to_owned(),
                        scheme_end: Some(scheme.len()),
                        authority_end: if authority_end > 0 { Some(authority_end) } else { None },
                        query: if query_len > 0 { Some(query_len) } else { None },
                    })
                }
            }
        }
    }

    pub fn path(&self) -> &str {
        let index = self.authority_end.unwrap_or(self.scheme_end.unwrap_or(0));
        let query_len = self.query.unwrap_or(0);
        let end = self.source.len() - if query_len > 0 { query_len + 1 } else { 0 };
        &self.source[index..end]
    }

    pub fn scheme(&self) -> Option<&str> {
        if let Some(end) = self.scheme_end {
            Some(&self.source[..end])
        } else {
            None
        }
    }

    pub fn authority(&self) -> Option<&str> {
        if let Some(end) = self.authority_end {
            let index = self.scheme_end.map(|i| i + 3).unwrap_or(0);
            Some(&self.source[index..end])
        } else {
            None
        }
    }

    pub fn query(&self) -> Option<&str> {
        if let Some(len) = self.query {
            Some(&self.source[self.source.len() - len..])
        } else {
            None
        }
    }
}

impl FromStr for Uri {
    type Err = Error;

    fn from_str(s: &str) -> Result<Uri, Error> {
        Uri::new(s)
    }
}

impl Default for Uri {
    fn default() -> Uri {
        Uri::new("*").unwrap()
    }
}

impl Display for Uri {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.source)
    }
}

#[test]
fn test_uri() {
    let uri = Uri::new("http://test.com/nazghul?test=3").expect("Uri::new failed");
    assert_eq!(uri.path(), "/nazghul");
    assert_eq!(uri.authority(), Some("test.com"));
    assert_eq!(uri.scheme(), Some("http"));
    assert_eq!(uri.query(), Some("test=3"));
}

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
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub enum RequestUri {
    /// The most common request target, an absolute path and optional query.
    ///
    /// For example, the line `GET /where?q=now HTTP/1.1` would parse the URI
    /// as `AbsolutePath { path: "/where".to_string(), query: Some("q=now".to_string()) }`.
    AbsolutePath {
        /// The path part of the request uri.
        path: String,
        /// The query part of the request uri.
        query: Option<String>,
    },

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

impl Default for RequestUri {
    fn default() -> RequestUri {
        RequestUri::Star
    }
}

impl FromStr for RequestUri {
    type Err = Error;

    fn from_str(s: &str) -> Result<RequestUri, Error> {
        let bytes = s.as_bytes();
        if bytes.len() == 0 {
            Err(Error::Uri(UrlError::RelativeUrlWithoutBase))
        } else if bytes == b"*" {
            Ok(RequestUri::Star)
        } else if bytes.starts_with(b"/") {
            let mut temp = "http://example.com".to_owned();
            temp.push_str(s);
            let url = try!(Url::parse(&temp));
            Ok(RequestUri::AbsolutePath {
                path: url.path().to_owned(),
                query: url.query().map(|q| q.to_owned()),
            })
        } else if bytes.contains(&b'/') {
            Ok(RequestUri::AbsoluteUri(try!(Url::parse(s))))
        } else {
            let mut temp = "http://".to_owned();
            temp.push_str(s);
            let url = try!(Url::parse(&temp));
            if url.query().is_some() {
                return Err(Error::Uri(UrlError::RelativeUrlWithoutBase));
            }
            //TODO: compare vs u.authority()?
            Ok(RequestUri::Authority(s.to_owned()))
        }
    }
}

impl Display for RequestUri {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            RequestUri::AbsolutePath { ref path, ref query } => {
                try!(f.write_str(path));
                match *query {
                    Some(ref q) => write!(f, "?{}", q),
                    None => Ok(()),
                }
            }
            RequestUri::AbsoluteUri(ref url) => write!(f, "{}", url),
            RequestUri::Authority(ref path) => f.write_str(path),
            RequestUri::Star => f.write_str("*")
        }
    }
}

#[test]
fn test_uri_fromstr() {
    fn parse(s: &str, result: RequestUri) {
        assert_eq!(s.parse::<RequestUri>().unwrap(), result);
    }
    fn parse_err(s: &str) {
        assert!(s.parse::<RequestUri>().is_err());
    }

    parse("*", RequestUri::Star);
    parse("**", RequestUri::Authority("**".to_owned()));
    parse("http://hyper.rs/", RequestUri::AbsoluteUri(Url::parse("http://hyper.rs/").unwrap()));
    parse("hyper.rs", RequestUri::Authority("hyper.rs".to_owned()));
    parse_err("hyper.rs?key=value");
    parse_err("hyper.rs/");
    parse("/", RequestUri::AbsolutePath { path: "/".to_owned(), query: None });
}

#[test]
fn test_uri_display() {
    fn assert_display(expected_string: &str, request_uri: RequestUri) {
        assert_eq!(expected_string, format!("{}", request_uri));
    }

    assert_display("*", RequestUri::Star);
    assert_display("http://hyper.rs/", RequestUri::AbsoluteUri(Url::parse("http://hyper.rs/").unwrap()));
    assert_display("hyper.rs", RequestUri::Authority("hyper.rs".to_owned()));
    assert_display("/", RequestUri::AbsolutePath { path: "/".to_owned(), query: None });
    assert_display("/where?key=value", RequestUri::AbsolutePath {
        path: "/where".to_owned(),
        query: Some("key=value".to_owned()),
    });
}
