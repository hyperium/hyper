use header::{Header, HeaderFormat};
use std::fmt;
use std::str::from_utf8;

use cookie::Cookie as CookiePair;
use cookie::CookieJar;

/// The `Cookie` header. Defined in [RFC6265](tools.ietf.org/html/rfc6265#section-5.4):
///
/// > If the user agent does attach a Cookie header field to an HTTP
/// > request, the user agent must send the cookie-string
/// > as the value of the header field.
///
/// > When the user agent generates an HTTP request, the user agent MUST NOT
/// > attach more than one Cookie header field.
#[derive(Clone, PartialEq, Debug)]
pub struct Cookie(pub Vec<CookiePair>);

deref!(Cookie => Vec<CookiePair>);

impl Header for Cookie {
    fn header_name() -> &'static str {
        "Cookie"
    }

    fn parse_header(raw: &[Vec<u8>]) -> Option<Cookie> {
        let mut cookies = Vec::with_capacity(raw.len());
        for cookies_raw in raw.iter() {
            match from_utf8(&cookies_raw[]) {
                Ok(cookies_str) => {
                    for cookie_str in cookies_str.split(';') {
                        match cookie_str.trim().parse() {
                            Some(cookie) => cookies.push(cookie),
                            None => return None
                        }
                    }
                },
                Err(_) => return None
            };
        }

        if !cookies.is_empty() {
            Some(Cookie(cookies))
        } else {
            None
        }
    }
}

impl HeaderFormat for Cookie {
    fn fmt_header(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let cookies = &self.0;
        let last = cookies.len() - 1;
        for (i, cookie) in cookies.iter().enumerate() {
            try!(write!(fmt, "{}", cookie.pair()));
            if i < last {
                try!(fmt.write_str("; "));
            }
        }
        Ok(())
    }
}

impl Cookie {
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
    pub fn from_cookie_jar(jar: &CookieJar) -> Cookie {
        Cookie(jar.iter().collect())
    }
}


#[test]
fn test_parse() {
    let h = Header::parse_header(&[b"foo=bar; baz=quux".to_vec()][]);
    let c1 = CookiePair::new("foo".to_string(), "bar".to_string());
    let c2 = CookiePair::new("baz".to_string(), "quux".to_string());
    assert_eq!(h, Some(Cookie(vec![c1, c2])));
}

#[test]
fn test_fmt() {
    use header::Headers;

    let mut cookie_pair = CookiePair::new("foo".to_string(), "bar".to_string());
    cookie_pair.httponly = true;
    cookie_pair.path = Some("/p".to_string());
    let cookie_header = Cookie(vec![cookie_pair, CookiePair::new("baz".to_string(), "quux".to_string())]);
    let mut headers = Headers::new();
    headers.set(cookie_header);

    assert_eq!(&headers.to_string()[], "Cookie: foo=bar; baz=quux\r\n");
}

#[test]
fn cookie_jar() {
    let cookie_pair = CookiePair::new("foo".to_string(), "bar".to_string());
    let cookie_header = Cookie(vec![cookie_pair]);
    let jar = cookie_header.to_cookie_jar(&[]);
    let new_cookie_header = Cookie::from_cookie_jar(&jar);

    assert_eq!(cookie_header, new_cookie_header);
}


bench_header!(bench, Cookie, { vec![b"foo=bar; baz=quux".to_vec()] });
