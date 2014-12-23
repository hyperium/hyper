use header::{Header, HeaderFormat};
use std::fmt::{mod, Show};
use std::str::{from_utf8, from_str};

use cookie::Cookie;
use cookie::CookieJar;

/// The `Cookie` header. Defined in [RFC6265](tools.ietf.org/html/rfc6265#section-5.4):
///
/// > If the user agent does attach a Cookie header field to an HTTP
/// > request, the user agent must send the cookie-string
/// > as the value of the header field.
///
/// > When the user agent generates an HTTP request, the user agent MUST NOT
/// > attach more than one Cookie header field.
#[deriving(Clone, PartialEq, Show)]
pub struct Cookies(pub Vec<Cookie>);

deref!(Cookies -> Vec<Cookie>);

impl Header for Cookies {
    fn header_name(_: Option<Cookies>) -> &'static str {
        "Cookie"
    }

    fn parse_header(raw: &[Vec<u8>]) -> Option<Cookies> {
        let mut cookies = Vec::with_capacity(raw.len());
        for cookies_raw in raw.iter() {
            match from_utf8(cookies_raw[]) {
                Ok(cookies_str) => {
                    for cookie_str in cookies_str.split(';') {
                        match from_str(cookie_str.trim()) {
                            Some(cookie) => cookies.push(cookie),
                            None => return None
                        }
                    }
                },
                Err(_) => return None
            };
        }

        if !cookies.is_empty() {
            Some(Cookies(cookies))
        } else {
            None
        }
    }
}

impl HeaderFormat for Cookies {
    fn fmt_header(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let cookies = &self.0;
        let last = cookies.len() - 1;
        for (i, cookie) in cookies.iter().enumerate() {
            try!(cookie.pair().fmt(fmt));
            if i < last {
                try!("; ".fmt(fmt));
            }
        }
        Ok(())
    }
}

impl Cookies {
    /// This method can be used to create CookieJar that can be used
    /// to manipulate cookies and create a corresponding `SetCookie` header afterwards.
    pub fn to_cookie_jar(&self, key: &[u8]) -> CookieJar<'static> {
        let mut jar = CookieJar::new(key);
        for cookie in self.iter() {
            jar.add_original(cookie.clone());
        }
        jar
    }

    /// Extracts all cookies from `CookieJar` and creates Cookie header.
    /// Useful for clients.
    pub fn from_cookie_jar(jar: &CookieJar) -> Cookies {
        Cookies(jar.iter().collect())
    }
}


#[test]
fn test_parse() {
    let h = Header::parse_header([b"foo=bar; baz=quux".to_vec()][]);
    let c1 = Cookie::new("foo".to_string(), "bar".to_string());
    let c2 = Cookie::new("baz".to_string(), "quux".to_string());
    assert_eq!(h, Some(Cookies(vec![c1, c2])));
}

#[test]
fn test_fmt() {
    use header::Headers;

    let mut cookie = Cookie::new("foo".to_string(), "bar".to_string());
    cookie.httponly = true;
    cookie.path = Some("/p".to_string());
    let cookies = Cookies(vec![cookie, Cookie::new("baz".to_string(), "quux".to_string())]);
    let mut headers = Headers::new();
    headers.set(cookies);

    assert_eq!(headers.to_string()[], "Cookie: foo=bar; baz=quux\r\n");
}

#[test]
fn cookie_jar() {
    let cookie = Cookie::new("foo".to_string(), "bar".to_string());
    let cookies = Cookies(vec![cookie]);
    let jar = cookies.to_cookie_jar(&[]);
    let new_cookies = Cookies::from_cookie_jar(&jar);

    assert_eq!(cookies, new_cookies);
}


bench_header!(bench, Cookies, { vec![b"foo=bar; baz=quux".to_vec()] });

