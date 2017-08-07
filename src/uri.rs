use std::error::Error as StdError;
use std::fmt::{Display, self};
use std::str::{self, FromStr};

use ::common::ByteStr;
use bytes::{BufMut, Bytes, BytesMut};

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
#[derive(Clone, Hash)]
pub struct Uri {
    source: ByteStr,
    scheme_end: Option<usize>,
    authority_end: Option<usize>,
    query_start: Option<usize>,
    fragment_start: Option<usize>,
}

impl Uri {
    /// Parse a string into a `Uri`.
    fn new(s: ByteStr) -> Result<Uri, UriError> {
        if s.len() == 0 {
            Err(UriError(ErrorKind::Empty))
        } else if s.as_bytes() == b"*" {
            // asterisk-form
            Ok(asterisk_form())
        } else if s.as_bytes() == b"/" {
            // shortcut for '/'
            Ok(Uri::default())
        } else if s.as_bytes()[0] == b'/' {
            // origin-form
            let query = parse_query(&s);
            let fragment = parse_fragment(&s);
            Ok(Uri {
                source: s,
                scheme_end: None,
                authority_end: None,
                query_start: query,
                fragment_start: fragment,
            })
        } else if s.contains("://") {
            // absolute-form
            let scheme = parse_scheme(&s);
            let auth = Some(parse_authority(&s));
            let scheme_end = scheme.expect("just checked for ':' above");
            let auth_end = auth.expect("just checked for ://");
            if scheme_end + 3 == auth_end {
                // authority was empty
                return Err(UriError(ErrorKind::MissingAuthority));
            }
            {
                let authority = &s.as_bytes()[scheme_end + 3..auth_end];
                let has_start_bracket = authority.contains(&b'[');
                let has_end_bracket = authority.contains(&b']');
                if has_start_bracket ^ has_end_bracket {
                    // has only 1 of [ and ]
                    return Err(UriError(ErrorKind::Malformed));
                }
            }
            let query = parse_query(&s);
            let fragment = parse_fragment(&s);
            Ok(Uri {
                source: s,
                scheme_end: scheme,
                authority_end: auth,
                query_start: query,
                fragment_start: fragment,
            })
        } else if (s.contains("/") || s.contains("?")) && !s.contains("://") {
            // last possibility is authority-form, above are illegal characters
            Err(UriError(ErrorKind::Malformed))
        } else {
            // authority-form
            let len = s.len();
            Ok(Uri {
                source: s,
                scheme_end: None,
                authority_end: Some(len),
                query_start: None,
                fragment_start: None,
            })
        }
    }

    /// Get the path of this `Uri`.
    #[inline]
    pub fn path(&self) -> &str {
        let index = self.path_start();
        let end = self.path_end();
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

    #[inline]
    fn path_start(&self) -> usize {
        self.authority_end.unwrap_or(self.scheme_end.unwrap_or(0))
    }

    #[inline]
    fn path_end(&self) -> usize {
        if let Some(query) = self.query_start {
            query
        } else if let Some(fragment) = self.fragment_start {
            fragment
        } else {
            self.source.len()
        }
    }

    #[inline]
    fn origin_form_end(&self) -> usize {
        if let Some(fragment) = self.fragment_start {
            fragment
        } else {
            self.source.len()
        }
    }

    /// Get the scheme of this `Uri`.
    #[inline]
    pub fn scheme(&self) -> Option<&str> {
        if let Some(end) = self.scheme_end {
            Some(&self.source[..end])
        } else {
            None
        }
    }

    /// Get the authority of this `Uri`.
    #[inline]
    pub fn authority(&self) -> Option<&str> {
        if let Some(end) = self.authority_end {
            let index = self.scheme_end.map(|i| i + 3).unwrap_or(0);

            Some(&self.source[index..end])
        } else {
            None
        }
    }

    /// Get the host of this `Uri`.
    #[inline]
    pub fn host(&self) -> Option<&str> {
        self.authority().map(|auth| {
            let host_port = auth.rsplit('@')
                .next()
                .expect("split always has at least 1 item");
            if host_port.as_bytes()[0] == b'[' {
                let i = host_port.find(']')
                    .expect("parsing should validate matching brackets");
                &host_port[1..i]
            } else {
                host_port.split(':')
                    .next()
                    .expect("split always has at least 1 item")
            }
        })
    }

    /// Get the port of this `Uri`.
    #[inline]
    pub fn port(&self) -> Option<u16> {
        match self.authority() {
            Some(auth) => auth.rfind(':').and_then(|i| auth[i+1..].parse().ok()),
            None => None,
       }
    }

    /// Get the query string of this `Uri`, starting after the `?`.
    #[inline]
    pub fn query(&self) -> Option<&str> {
        self.query_start.map(|start| {
            // +1 to remove '?'
            let start = start + 1;
            if let Some(end) = self.fragment_start {
                &self.source[start..end]
            } else {
                &self.source[start..]
            }
        })
    }

    /// Returns whether this URI is in `absolute-form`.
    ///
    /// An example of absolute form is `https://hyper.rs`.
    #[inline]
    pub fn is_absolute(&self) -> bool {
        self.scheme_end.is_some()
    }

    #[cfg(test)]
    fn fragment(&self) -> Option<&str> {
        self.fragment_start.map(|start| {
            // +1 to remove the '#'
           &self.source[start + 1..]
        })
    }
}

fn parse_scheme(s: &str) -> Option<usize> {
    s.find(':')
}

fn parse_authority(s: &str) -> usize {
    let i = s.find("://").map(|p| p + 3).unwrap_or(0);
    s[i..]
        .find(|ch| ch == '/' || ch == '?' || ch == '#')
        .map(|end| end + i)
        .unwrap_or(s.len())
}

fn parse_query(s: &str) -> Option<usize> {
    s.find('?').and_then(|i| {
        if let Some(frag) = s.find('#') {
            if frag < i {
                None
            } else {
                Some(i)
            }
        } else {
            Some(i)
        }
    })
}

fn parse_fragment(s: &str) -> Option<usize> {
    s.find('#')
}

impl FromStr for Uri {
    type Err = UriError;

    fn from_str(s: &str) -> Result<Uri, UriError> {
        //TODO: refactor such that the to_owned() is only required at the end
        //of successful parsing, so an Err doesn't needlessly clone the string.
        Uri::new(ByteStr::from(s))
    }
}

impl PartialEq for Uri {
    fn eq(&self, other: &Uri) -> bool {
        self.source.as_str() == other.source.as_str()
    }
}

impl<'a> PartialEq<&'a str> for Uri {
    fn eq(&self, other: & &'a str) -> bool {
        self.source.as_str() == *other
    }
}

impl<'a> PartialEq<Uri> for &'a str{
    fn eq(&self, other: &Uri) -> bool {
        *self == other.source.as_str()
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
            source: ByteStr::from_static("/"),
            scheme_end: None,
            authority_end: None,
            query_start: None,
            fragment_start: None,
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

pub unsafe fn from_utf8_unchecked(slice: Bytes) -> Result<Uri, UriError> {
    Uri::new(ByteStr::from_utf8_unchecked(slice))
}

pub fn scheme_and_authority(uri: &Uri) -> Option<Uri> {
    if uri.scheme_end.is_some() {
        Some(Uri {
            source: uri.source.slice_to(uri.authority_end.expect("scheme without authority")),
            scheme_end: uri.scheme_end,
            authority_end: uri.authority_end,
            query_start: None,
            fragment_start: None,
        })
    } else {
        None
    }
}

#[inline]
fn asterisk_form() -> Uri {
    Uri {
        source: ByteStr::from_static("*"),
        scheme_end: None,
        authority_end: None,
        query_start: None,
        fragment_start: None,
    }
}

pub fn origin_form(uri: &Uri) -> Uri {
    let range = Range(uri.path_start(), uri.origin_form_end());

    let clone = if range.len() == 0 {
        ByteStr::from_static("/")
    } else if uri.source.as_bytes()[range.0] == b'*' {
        return asterisk_form();
    } else if uri.source.as_bytes()[range.0] != b'/' {
        let mut new = BytesMut::with_capacity(range.1 - range.0 + 1);
        new.put_u8(b'/');
        new.put_slice(&uri.source.as_bytes()[range.0..range.1]);
        // safety: the bytes are '/' + previous utf8 str
        unsafe { ByteStr::from_utf8_unchecked(new.freeze()) }
    } else if range.0 == 0 && range.1 == uri.source.len() {
        uri.source.clone()
    } else {
        uri.source.slice(range.0, range.1)
    };

    Uri {
        source: clone,
        scheme_end: None,
        authority_end: None,
        query_start: uri.query_start,
        fragment_start: None,
    }
}

struct Range(usize, usize);

impl Range {
    fn len(&self) -> usize {
        self.1 - self.0
    }
}

/// An error parsing a `Uri`.
#[derive(Clone, Debug)]
pub struct UriError(ErrorKind);

#[derive(Clone, Debug)]
enum ErrorKind {
    Empty,
    Malformed,
    MissingAuthority,
}

impl fmt::Display for UriError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad(self.description())
    }
}

impl StdError for UriError {
    fn description(&self) -> &str {
        match self.0 {
            ErrorKind::Empty => "empty Uri string",
            ErrorKind::Malformed => "invalid character in Uri authority",
            ErrorKind::MissingAuthority => "absolute Uri missing authority segment",
        }
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
            println!("{:?} = {:#?}", $str, uri);
            $(
            assert_eq!(uri.$method(), $value, stringify!($method));
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
    host = Some("127.0.0.1"),
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
    host = Some("127.0.0.1"),
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
    host = Some("localhost"),
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
    host = Some("localhost"),
    path = "",
    query = None,
    fragment = None,
    port = Some(3000),
}

test_parse! {
    test_uri_parse_absolute_with_default_port_http,
    "http://127.0.0.1:80/foo",

    scheme = Some("http"),
    authority = Some("127.0.0.1:80"),
    host = Some("127.0.0.1"),
    path = "/foo",
    query = None,
    fragment = None,
    port = Some(80),
}

test_parse! {
    test_uri_parse_absolute_with_default_port_https,
    "https://127.0.0.1:443",

    scheme = Some("https"),
    authority = Some("127.0.0.1:443"),
    host = Some("127.0.0.1"),
    path = "/",
    query = None,
    fragment = None,
    port = Some(443),
}

test_parse! {
    test_uri_parse_absolute_with_ipv6,
    "https://[2001:0db8:85a3:0000:0000:8a2e:0370:7334]:8008",

    scheme = Some("https"),
    authority = Some("[2001:0db8:85a3:0000:0000:8a2e:0370:7334]:8008"),
    host = Some("2001:0db8:85a3:0000:0000:8a2e:0370:7334"),
    path = "/",
    query = None,
    fragment = None,
    port = Some(8008),
}

test_parse! {
    test_uri_parse_absolute_with_ipv6_and_no_port,
    "https://[2001:0db8:85a3:0000:0000:8a2e:0370:7334]",

    scheme = Some("https"),
    authority = Some("[2001:0db8:85a3:0000:0000:8a2e:0370:7334]"),
    host = Some("2001:0db8:85a3:0000:0000:8a2e:0370:7334"),
    path = "/",
    query = None,
    fragment = None,
    port = None,
}

test_parse! {
    test_uri_parse_absolute_with_userinfo,
    "https://seanmonstar:password@hyper.rs",

    scheme = Some("https"),
    authority = Some("seanmonstar:password@hyper.rs"),
    host = Some("hyper.rs"),
    path = "/",
    query = None,
    fragment = None,
    port = None,
}

test_parse! {
    test_uri_parse_fragment_questionmark,
    "http://127.0.0.1/#?",

    scheme = Some("http"),
    authority = Some("127.0.0.1"),
    host = Some("127.0.0.1"),
    path = "/",
    query = None,
    fragment = Some("?"),
    port = None,
}

test_parse! {
    test_uri_parse_path_with_terminating_questionmark,
    "http://127.0.0.1/path?",

    scheme = Some("http"),
    authority = Some("127.0.0.1"),
    host = Some("127.0.0.1"),
    path = "/path",
    query = Some(""),
    fragment = None,
    port = None,
}

test_parse! {
    test_uri_parse_absolute_form_with_empty_path_and_nonempty_query,
    "http://127.0.0.1?foo=bar",

    scheme = Some("http"),
    authority = Some("127.0.0.1"),
    host = Some("127.0.0.1"),
    path = "/",
    query = Some("foo=bar"),
    fragment = None,
    port = None,
}

test_parse! {
    test_uri_parse_absolute_form_with_empty_path_and_fragment_with_slash,
    "http://127.0.0.1#foo/bar",
    scheme = Some("http"),
    authority = Some("127.0.0.1"),
    host = Some("127.0.0.1"),
    path = "/",
    query = None,
    fragment = Some("foo/bar"),
    port = None,
}

test_parse! {
    test_uri_parse_absolute_form_with_empty_path_and_fragment_with_questionmark,
    "http://127.0.0.1#foo?bar",
    scheme = Some("http"),
    authority = Some("127.0.0.1"),
    host = Some("127.0.0.1"),
    path = "/",
    query = None,
    fragment = Some("foo?bar"),
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
    err("?key=val");
    err("localhost/");
    err("localhost?key=val");
    err("http://::1]");
    err("http://[::1");
}

#[test]
fn test_uri_to_origin_form() {
    let cases = vec![
        ("/", "/"),
        ("/foo?bar", "/foo?bar"),
        ("/foo?bar#nope", "/foo?bar"),
        ("http://hyper.rs", "/"),
        ("http://hyper.rs/", "/"),
        ("http://hyper.rs/path", "/path"),
        ("http://hyper.rs?query", "/?query"),
        ("*", "*"),
    ];

    for case in cases {
        let uri = Uri::from_str(case.0).unwrap();
        assert_eq!(origin_form(&uri), case.1); //, "{:?}", case);
    }
}
