use header::{Header, HeaderFormat};
use std::fmt::{self, Show};
use std::str::from_utf8;

use cookie::Cookie;
use cookie::CookieJar;

/// The `Set-Cookie` header
///
/// Informally, the Set-Cookie response header contains the header name
/// "Set-Cookie" followed by a ":" and a cookie.  Each cookie begins with
/// a name-value-pair, followed by zero or more attribute-value pairs.
#[derive(Clone, PartialEq, Show)]
pub struct SetCookie(pub Vec<Cookie>);

//TODO: remove when fixed in libstd
unsafe impl Send for SetCookie {}
unsafe impl Sync for SetCookie {}

deref!(SetCookie => Vec<Cookie>);

impl Header for SetCookie {
    fn header_name(_: Option<SetCookie>) -> &'static str {
        "Set-Cookie"
    }

    fn parse_header(raw: &[Vec<u8>]) -> Option<SetCookie> {
        let mut set_cookies = Vec::with_capacity(raw.len());
        for set_cookies_raw in raw.iter() {
            match from_utf8(&set_cookies_raw[]) {
                Ok(s) if !s.is_empty() => {
                    match s.parse() {
                        Some(cookie) => set_cookies.push(cookie),
                        None => ()
                    }
                },
                _ => ()
            };
        }

        if !set_cookies.is_empty() {
            Some(SetCookie(set_cookies))
        } else {
            None
        }
    }

}

impl HeaderFormat for SetCookie {

    fn fmt_header(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for (i, cookie) in self.0.iter().enumerate() {
            if i != 0 {
                try!(f.write_str("\r\nSet-Cookie: "));
            }
            try!(cookie.fmt(f));
        }
        Ok(())
    }
}


impl SetCookie {
    /// Use this to create SetCookie header from CookieJar using
    /// calculated delta.
    pub fn from_cookie_jar(jar: &CookieJar) -> SetCookie {
        SetCookie(jar.delta())
    }

    /// Use this on client to apply changes from SetCookie to CookieJar.
    /// Note that this will `panic!` if `CookieJar` is not root.
    pub fn apply_to_cookie_jar(&self, jar: &mut CookieJar) {
        for cookie in self.iter() {
            jar.add_original(cookie.clone())
        }
    }
}


#[test]
fn test_parse() {
    let h = Header::parse_header(&[b"foo=bar; HttpOnly".to_vec()][]);
    let mut c1 = Cookie::new("foo".to_string(), "bar".to_string());
    c1.httponly = true;

    assert_eq!(h, Some(SetCookie(vec![c1])));
}

#[test]
fn test_fmt() {
    use header::Headers;

    let mut cookie = Cookie::new("foo".to_string(), "bar".to_string());
    cookie.httponly = true;
    cookie.path = Some("/p".to_string());
    let cookies = SetCookie(vec![cookie, Cookie::new("baz".to_string(), "quux".to_string())]);
    let mut headers = Headers::new();
    headers.set(cookies);

    assert_eq!(&headers.to_string()[], "Set-Cookie: foo=bar; HttpOnly; Path=/p\r\nSet-Cookie: baz=quux; Path=/\r\n");
}

#[test]
fn cookie_jar() {
    let jar = CookieJar::new(b"secret");
    let cookie = Cookie::new("foo".to_string(), "bar".to_string());
    jar.encrypted().add(cookie);

    let cookies = SetCookie::from_cookie_jar(&jar);

    let mut new_jar = CookieJar::new(b"secret");
    cookies.apply_to_cookie_jar(&mut new_jar);

    assert_eq!(jar.encrypted().find("foo"), new_jar.encrypted().find("foo"));
    assert_eq!(jar.iter().collect::<Vec<Cookie>>(), new_jar.iter().collect::<Vec<Cookie>>());
}
