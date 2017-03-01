use std::borrow::Cow;
use std::fmt::{Display, self};
use std::ops::Deref;
use std::str::{self, FromStr};

use http::ByteStr;
use Url;
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
/// ```notrust
/// abc://username:password@example.com:123/path/data?key=value&key2=value2#fragid1
/// |-|   |-------------------------------||--------| |-------------------| |-----|
///  |                  |                       |               |              |
/// scheme          authority                 path            query         fragment
/// ```
#[derive(Clone)]
pub struct Uri {
    source: InternalUri,
    scheme_end: Option<usize>,
    authority_end: Option<usize>,
    query: Option<usize>,
    fragment: Option<usize>,
}

impl Uri {
    /// Parse a string into a `Uri`.
    fn new(s: InternalUri) -> Result<Uri, Error> {
        if s.len() == 0 {
            Err(Error::Uri(UrlError::RelativeUrlWithoutBase))
        } else if s.as_bytes() == b"*" {
            Ok(Uri {
                source: InternalUri::Cow("*".into()),
                scheme_end: None,
                authority_end: None,
                query: None,
                fragment: None,
            })
        } else if s.as_bytes() == b"/" {
            Ok(Uri::default())
        } else if s.as_bytes()[0] == b'/' {
            let query = parse_query(&s);
            let fragment = parse_fragment(&s);
            Ok(Uri {
                source: s,
                scheme_end: None,
                authority_end: None,
                query: query,
                fragment: fragment,
            })
        } else if s.contains("://") {
            let scheme = parse_scheme(&s);
            let auth = parse_authority(&s);
            if let Some(end) = scheme {
                match &s[..end] {
                    "ftp" | "gopher" | "http" | "https" | "ws" | "wss" => {},
                    "blob" | "file" => return Err(Error::Method),
                    _ => return Err(Error::Method),
                }
                match auth {
                    Some(a) => {
                        if (end + 3) == a {
                            return Err(Error::Method);
                        }
                    },
                    None => return Err(Error::Method),
                }
            }
            let query = parse_query(&s);
            let fragment = parse_fragment(&s);
            Ok(Uri {
                source: s,
                scheme_end: scheme,
                authority_end: auth,
                query: query,
                fragment: fragment,
            })
        } else if (s.contains("/") || s.contains("?")) && !s.contains("://") {
            return Err(Error::Method)
        } else {
            let len = s.len();
            Ok(Uri {
                source: s,
                scheme_end: None,
                authority_end: Some(len),
                query: None,
                fragment: None,
            })
        }
    }

    /// Get the path of this `Uri`.
    pub fn path(&self) -> &str {
        let index = self.authority_end.unwrap_or(self.scheme_end.unwrap_or(0));
        let query_len = self.query.unwrap_or(0);
        let fragment_len = self.fragment.unwrap_or(0);
        let end = self.source.len() - if query_len > 0 { query_len + 1 } else { 0 } -
            if fragment_len > 0 { fragment_len + 1 } else { 0 };
        if index >= end {
            if self.scheme().is_some() {
                "/" // absolute-form MUST have path
            } else {
                ""
            }
        } else {
            &self.source[index..end]
        }
    }

    /// Get the scheme of this `Uri`.
    pub fn scheme(&self) -> Option<&str> {
        if let Some(end) = self.scheme_end {
            Some(&self.source[..end])
        } else {
            None
        }
    }

    /// Get the authority of this `Uri`.
    pub fn authority(&self) -> Option<&str> {
        if let Some(end) = self.authority_end {
            let index = self.scheme_end.map(|i| i + 3).unwrap_or(0);

            Some(&self.source[index..end])
        } else {
            None
        }
    }

    /// Get the host of this `Uri`.
    pub fn host(&self) -> Option<&str> {
        if let Some(auth) = self.authority() {
            auth.split(":").next()
        } else {
            None
        }
    }

    /// Get the port of this `Uri`.
    pub fn port(&self) -> Option<u16> {
        match self.authority() {
            Some(auth) => auth.find(":").and_then(|i| u16::from_str(&auth[i+1..]).ok()),
            None => None,
       }
    }

    /// Get the query string of this `Uri`, starting after the `?`.
    pub fn query(&self) -> Option<&str> {
        let fragment_len = self.fragment.unwrap_or(0);
        let fragment_len = if fragment_len > 0 { fragment_len + 1 } else { 0 };
        if let Some(len) = self.query {
            Some(&self.source[self.source.len() - len - fragment_len..
                                       self.source.len() - fragment_len])
        } else {
            None
        }
    }

    #[cfg(test)]
    fn fragment(&self) -> Option<&str> {
        if let Some(len) = self.fragment {
            Some(&self.source[self.source.len() - len..self.source.len()])
        } else {
            None
        }
    }
}

fn parse_scheme(s: &str) -> Option<usize> {
    s.find(':')
}

fn parse_authority(s: &str) -> Option<usize> {
    let i = s.find("://").and_then(|p| Some(p + 3)).unwrap_or(0);

    Some(&s[i..].split("/")
         .next()
         .unwrap_or(s)
         .len() + i)
}

fn parse_query(s: &str) -> Option<usize> {
    match s.find('?') {
        Some(i) => {
            let frag_pos = s.find('#').unwrap_or(s.len());

            if frag_pos < i + 1 {
                None
            } else {
                Some(frag_pos - i - 1)
            }
        },
        None => None,
    }
}

fn parse_fragment(s: &str) -> Option<usize> {
    match s.find('#') {
        Some(i) => Some(s.len() - i - 1),
        None => None,
    }
}

impl FromStr for Uri {
    type Err = Error;

    fn from_str(s: &str) -> Result<Uri, Error> {
        //TODO: refactor such that the to_owned() is only required at the end
        //of successful parsing, so an Err doesn't needlessly clone the string.
        Uri::new(InternalUri::Cow(Cow::Owned(s.to_owned())))
    }
}

impl From<Url> for Uri {
    fn from(url: Url) -> Uri {
        let s = InternalUri::Cow(Cow::Owned(url.into_string()));
        // TODO: we could build the indices with some simple subtraction,
        // instead of re-parsing an already parsed string.
        // For example:
        // let scheme_end = url[..url::Position::AfterScheme].as_ptr() as usize
        //   - url.as_str().as_ptr() as usize;
        // etc
        Uri::new(s).expect("Uri::From<Url> failed")
    }
}

impl PartialEq for Uri {
    fn eq(&self, other: &Uri) -> bool {
        self.source.as_str() == other.source.as_str()
    }
}

impl Eq for Uri {}

impl AsRef<str> for Uri {
    fn as_ref(&self) -> &str {
        self.source.as_str()
    }
}

impl Default for Uri {
    fn default() -> Uri {
        Uri {
            source: InternalUri::Cow("/".into()),
            scheme_end: None,
            authority_end: None,
            query: None,
            fragment: None,
        }
    }
}

impl fmt::Debug for Uri {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self.as_ref(), f)
    }
}

impl Display for Uri {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.as_ref())
    }
}

pub fn from_mem_str(s: ByteStr) -> Result<Uri, Error> {
    Uri::new(InternalUri::Shared(s))
}

#[derive(Clone)]
enum InternalUri {
    Cow(Cow<'static, str>),
    Shared(ByteStr),
}

impl InternalUri {
    fn as_str(&self) -> &str {
        match *self {
            InternalUri::Cow(ref s) => s.as_ref(),
            InternalUri::Shared(ref m) => m.as_str(),
        }
    }
}

impl Deref for InternalUri {
    type Target = str;

    fn deref(&self) -> &str {
        self.as_str()
    }
}


macro_rules! test_parse {
    (
        $test_name:ident,
        $str:expr,
        $($method:ident = $value:expr,)*
    ) => (
        #[test]
        fn $test_name() {
            let uri = Uri::from_str($str).unwrap();
            $(
            assert_eq!(uri.$method(), $value);
            )+
        }
    );
}

test_parse! {
    test_uri_parse_origin_form,
    "/some/path/here?and=then&hello#and-bye",

    scheme = None,
    authority = None,
    path = "/some/path/here",
    query = Some("and=then&hello"),
    fragment = Some("and-bye"),
}

test_parse! {
    test_uri_parse_absolute_form,
    "http://127.0.0.1:61761/chunks",

    scheme = Some("http"),
    authority = Some("127.0.0.1:61761"),
    path = "/chunks",
    query = None,
    fragment = None,
    port = Some(61761),
}

test_parse! {
    test_uri_parse_absolute_form_without_path,
    "https://127.0.0.1:61761",

    scheme = Some("https"),
    authority = Some("127.0.0.1:61761"),
    path = "/",
    query = None,
    fragment = None,
    port = Some(61761),
}

test_parse! {
    test_uri_parse_asterisk_form,
    "*",

    scheme = None,
    authority = None,
    path = "*",
    query = None,
    fragment = None,
}

test_parse! {
    test_uri_parse_authority_no_port,
    "localhost",

    scheme = None,
    authority = Some("localhost"),
    path = "",
    query = None,
    fragment = None,
    port = None,
}

test_parse! {
    test_uri_parse_authority_form,
    "localhost:3000",

    scheme = None,
    authority = Some("localhost:3000"),
    path = "",
    query = None,
    fragment = None,
    port = Some(3000),
}

test_parse! {
    test_uri_parse_absolute_with_default_port_http,
    "http://127.0.0.1:80",

    scheme = Some("http"),
    authority = Some("127.0.0.1:80"),
    path = "/",
    query = None,
    fragment = None,
    port = Some(80),
}

test_parse! {
    test_uri_parse_absolute_with_default_port_https,
    "https://127.0.0.1:443",

    scheme = Some("https"),
    authority = Some("127.0.0.1:443"),
    path = "/",
    query = None,
    fragment = None,
    port = Some(443),
}

test_parse! {
    test_uri_parse_fragment_questionmark,
    "http://127.0.0.1/#?",

    scheme = Some("http"),
    authority = Some("127.0.0.1"),
    path = "/",
    query = None,
    fragment = Some("?"),
    port = None,
}

#[test]
fn test_uri_parse_error() {
    fn err(s: &str) {
        Uri::from_str(s).unwrap_err();
    }

    err("http://");
    err("htt:p//host");
    err("hyper.rs/");
    err("hyper.rs?key=val");
    err("localhost/");
    err("localhost?key=val");
}

#[test]
fn test_uri_from_url() {
    let uri = Uri::from(Url::parse("http://test.com/nazghul?test=3").unwrap());
    assert_eq!(uri.path(), "/nazghul");
    assert_eq!(uri.authority(), Some("test.com"));
    assert_eq!(uri.scheme(), Some("http"));
    assert_eq!(uri.query(), Some("test=3"));
    assert_eq!(uri.as_ref(), "http://test.com/nazghul?test=3");
}
