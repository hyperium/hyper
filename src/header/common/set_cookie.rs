use header::{Header, HeaderFormat};
use std::fmt;
use std::str::from_utf8;

use header::CookiePair;

/// The `Set-Cookie` header
///
/// Informally, the Set-Cookie response header contains the header name
/// "Set-Cookie" followed by a ":" and a cookie.  Each cookie begins with
/// a name-value-pair, followed by zero or more attribute-value pairs.
#[derive(Clone, PartialEq, Debug)]
pub struct SetCookie(pub Vec<CookiePair>);

deref!(SetCookie => Vec<CookiePair>);

impl Header for SetCookie {
    fn header_name() -> &'static str {
        "Set-Cookie"
    }

    fn parse_header(raw: &[Vec<u8>]) -> Option<SetCookie> {
        let mut set_cookies = Vec::with_capacity(raw.len());
        for set_cookies_raw in raw.iter() {
            match from_utf8(&set_cookies_raw[..]) {
                Ok(s) if !s.is_empty() => {
                    match s.parse() {
                        Ok(cookie) => set_cookies.push(cookie),
                        Err(_) => ()
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
            try!(write!(f, "{}", cookie));
        }
        Ok(())
    }
}

#[test]
fn test_parse() {
    let h = Header::parse_header(&[b"foo=bar; HttpOnly".to_vec()][..]);
    let mut c1 = CookiePair::new("foo".to_string(), "bar".to_string());
    c1.httponly = true;

    assert_eq!(h, Some(SetCookie(vec![c1])));
}

#[test]
fn test_fmt() {
    use header::Headers;

    let mut cookie = CookiePair::new("foo".to_string(), "bar".to_string());
    cookie.httponly = true;
    cookie.path = Some("/p".to_string());
    let cookies = SetCookie(vec![cookie, CookiePair::new("baz".to_string(), "quux".to_string())]);
    let mut headers = Headers::new();
    headers.set(cookies);

    assert_eq!(&headers.to_string()[..], "Set-Cookie: foo=bar; HttpOnly; Path=/p\r\nSet-Cookie: baz=quux; Path=/\r\n");
}
