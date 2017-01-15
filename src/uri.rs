//! HTTP RequestUris
use std::fmt::{Display, self};
use std::str::FromStr;
use url::{self, Url};
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
///
/// # Uri explanations
///
/// abc://username:password@example.com:123/path/data?key=value&key2=value2#fragid1
/// |-|   |-------------------------------||--------| |-------------------| |-----|
///  |                  |                       |               |              |
/// scheme          authority                 path            query         fragment
#[derive(Debug, Clone)]
pub struct Uri {
    source: String,
    scheme_end: Option<usize>,
    authority_end: Option<usize>,
    query: Option<usize>,
    fragment: Option<usize>,
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
                fragment: None,
            })
        } else if bytes.starts_with(b"/") {
            let mut temp = "http://example.com".to_owned();
            temp.push_str(s);
            let url = try!(Url::parse(&temp));
            let query_len = url.query().unwrap_or("").len();
            let fragment_len = url.fragment().unwrap_or("").len();
            Ok(Uri {
                source: s.to_owned(),
                scheme_end: None,
                authority_end: None,
                query: if query_len > 0 { Some(query_len) } else { None },
                fragment: if fragment_len > 0 { Some(fragment_len) } else { None },
            })
        } else if s.contains("://") {
            let url = try!(Url::parse(s));
            let query_len = url.query().unwrap_or("").len();
            let v: Vec<&str> = s.split("://").collect();
            let authority_end = v.last().unwrap()
                                        .split(url.path())
                                        .next()
                                        .unwrap_or(s)
                                        .len() + if v.len() == 2 { v[0].len() + 3 } else { 0 };
            let fragment_len = url.fragment().unwrap_or("").len();
            match url.origin() {
                url::Origin::Opaque(_) => Err(Error::Method),
                url::Origin::Tuple(scheme, _, _) => {
                    Ok(Uri {
                        source: s.to_owned(),
                        scheme_end: Some(scheme.len()),
                        authority_end: if authority_end > 0 { Some(authority_end) } else { None },
                        query: if query_len > 0 { Some(query_len) } else { None },
                        fragment: if fragment_len > 0 { Some(fragment_len) } else { None },
                    })
                }
            }
        } else {
            Uri::new(&format!("http://{}", s))
        }
    }

    pub fn path(&self) -> &str {
        let index = self.authority_end.unwrap_or(self.scheme_end.unwrap_or(0));
        let query_len = self.query.unwrap_or(0);
        let fragment_len = self.fragment.unwrap_or(0);
        let end = self.source.len() - if query_len > 0 { query_len + 1 } else { 0 } -
            if fragment_len > 0 { fragment_len + 1 } else { 0 };
        if index >= end {
            "/"
        } else {
            &self.source[index..end]
        }
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

    pub fn host(&self) -> Option<&str> {
        if let Some(auth) = self.authority() {
            auth.split(":").next()
        } else {
            None
        }
    }

    pub fn port(&self) -> Option<u16> {
        if let Some(auth) = self.authority() {
            let v: Vec<&str> = auth.split(":").collect();
            if v.len() == 2 {
                u16::from_str(v[1]).ok()
            } else {
                None
            }
        } else {
            None
        }
    }

    pub fn query(&self) -> Option<&str> {
        let fragment_len = self.fragment.unwrap_or(0);
        let fragment_len = if fragment_len > 0 { fragment_len + 1 } else { 0 };
        if let Some(len) = self.query {
            Some(&self.source[self.source.len() - len - fragment_len..self.source.len() - fragment_len])
        } else {
            None
        }
    }

    pub fn fragment(&self) -> Option<&str> {
        if let Some(len) = self.fragment {
            Some(&self.source[self.source.len() - len..])
        } else {
            None
        }
    }

    pub fn uri(&self) -> &str {
        &self.source
    }
}

impl FromStr for Uri {
    type Err = Error;

    fn from_str(s: &str) -> Result<Uri, Error> {
        Uri::new(s)
    }
}

impl From<Url> for Uri {
    fn from(url: Url) -> Uri {
        Uri::new(url.as_str()).expect("Uri::From<Url> failed")
    }
}

impl PartialEq for Uri {
    fn eq(&self, other: &Uri) -> bool {
        self.source == other.source
    }
}

impl AsRef<str> for Uri {
    fn as_ref(&self) -> &str {
        &self.source
    }
}

impl Default for Uri {
    fn default() -> Uri {
        Uri::new("*").unwrap()
    }
}

impl Display for Uri {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.path())
    }
}

#[test]
fn test_uri() {
    let uri = Uri::new("http://test.com/nazghul?test=3#fragment").expect("Uri::new failed");
    assert_eq!(uri.path(), "/nazghul");
    assert_eq!(uri.authority(), Some("test.com"));
    assert_eq!(uri.scheme(), Some("http"));
    assert_eq!(uri.query(), Some("test=3"));
    assert_eq!(uri.host(), Some("test.com"));
    assert_eq!(uri.fragment(), Some("fragment"));

    let uri = Uri::new("http://127.0.0.1:61761/chunks").expect("Uri::new without fragment failed");
    assert_eq!(uri.path(), "/chunks");
    assert_eq!(uri.authority(), Some("127.0.0.1:61761"));
    assert_eq!(uri.scheme(), Some("http"));
    assert_eq!(uri.query(), None);
    assert_eq!(uri.host(), Some("127.0.0.1"));
    assert_eq!(uri.fragment(), None);

    let uri = Uri::new("http://127.0.0.1:61761").expect("Uri::new without path failed");
    assert_eq!(uri.path(), "/");
    assert_eq!(uri.authority(), Some("127.0.0.1:61761"));
    assert_eq!(uri.scheme(), Some("http"));
    assert_eq!(uri.query(), None);
    assert_eq!(uri.host(), Some("127.0.0.1"));
    assert_eq!(uri.fragment(), None);

    let uri = Uri::new("127.0.0.1:61761/").expect("Uri::new without scheme failed");
    assert_eq!(uri.path(), "/");
    assert_eq!(uri.authority(), Some("127.0.0.1:61761"));
    assert_eq!(uri.scheme(), Some("http"));
    assert_eq!(uri.query(), None);
    assert_eq!(uri.host(), Some("127.0.0.1"));
    assert_eq!(uri.fragment(), None);

    let uri = Uri::new("127.0.0.1:61761").expect("Uri::new without scheme and path failed");
    assert_eq!(uri.path(), "/");
    assert_eq!(uri.authority(), Some("127.0.0.1:61761"));
    assert_eq!(uri.scheme(), Some("http"));
    assert_eq!(uri.query(), None);
    assert_eq!(uri.host(), Some("127.0.0.1"));
    assert_eq!(uri.fragment(), None);

    let uri = Uri::new("/test").expect("Uri::new path only failed");
    assert_eq!(uri.path(), "/test");
    assert_eq!(uri.authority(), None);
    assert_eq!(uri.scheme(), None);
    assert_eq!(uri.query(), None);
    assert_eq!(uri.host(), None);
    assert_eq!(uri.fragment(), None);

    let uri = Uri::new("*").expect("Uri::new star failed");
    assert_eq!(uri.path(), "*");
    assert_eq!(uri.authority(), None);
    assert_eq!(uri.scheme(), None);
    assert_eq!(uri.query(), None);
    assert_eq!(uri.host(), None);
    assert_eq!(uri.fragment(), None);
}

#[test]
fn test_uri_from_url() {
    let uri = Uri::from(Url::parse("http://test.com/nazghul?test=3").unwrap());
    assert_eq!(uri.path(), "/nazghul");
    assert_eq!(uri.authority(), Some("test.com"));
    assert_eq!(uri.scheme(), Some("http"));
    assert_eq!(uri.query(), Some("test=3"));
    assert_eq!(uri.uri(), "http://test.com/nazghul?test=3");
}
