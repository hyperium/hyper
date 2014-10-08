use header::Header;
use std::fmt;
use std::str::from_utf8;
use cookie::CookieJar;

/// The `Set-Cookie` header
///
/// Informally, the Set-Cookie response header contains the header name
/// "Set-Cookie" followed by a ":" and a cookie.  Each cookie begins with
/// a name-value-pair, followed by zero or more attribute-value pairs.
#[deriving(Clone, PartialEq, Show)]
pub struct SetCookie(pub Vec<String>);

impl Header for SetCookie {
    fn header_name(_: Option<SetCookie>) -> &'static str {
        "Set-Cookie"
    }

    fn parse_header(raw: &[Vec<u8>]) -> Option<SetCookie> {
        let mut set_cookies: Vec<String> = vec![];
        for set_cookies_raw in raw.iter() {
            match from_utf8(set_cookies_raw.as_slice()) {
                Some(set_cookies_str) => {
                    if !set_cookies_str.is_empty() {
                        set_cookies.push(set_cookies_str.to_string());
                    }
                },
                None => ()
            };
        }

        if !set_cookies.is_empty() {
            Some(SetCookie(set_cookies))
        } else {
            None
        }
    }

    fn fmt_header(&self, _: &mut fmt::Formatter) -> fmt::Result {
        unimplemented!()
    }
}

impl SetCookie {
    /// Use this to crate SetCookie header from CookieJar using
    /// calculated delta.
    #[allow(dead_code)]
    fn from_cookie_jar(jar: &CookieJar) -> SetCookie {
        SetCookie(jar.delta())
    }
}

