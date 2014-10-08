use header::Header;
use std::fmt::{mod, Show};
use std::str::from_utf8;
use cookie::Cookie as CookieRs;
use cookie::CookieJar;

/// The `Cookie` header
///
/// If the user agent does attach a Cookie header field to an HTTP
/// request, the user agent must send the cookie-string
/// as the value of the header field.
///
/// When the user agent generates an HTTP request, the user agent MUST NOT 
/// attach more than one Cookie header field.
#[deriving(Clone, PartialEq, Show)]
pub struct Cookie(pub Vec<CookieRs>);

impl Header for Cookie {
    fn header_name(_: Option<Cookie>) -> &'static str {
        "Cookie"
    }

    fn parse_header(raw: &[Vec<u8>]) -> Option<Cookie> {
        let mut cookies: Vec<CookieRs> = vec![];
        for cookies_raw in raw.iter() {
            match from_utf8(cookies_raw.as_slice()) {
                Some(cookies_str) => {
                    for cookie_str in cookies_str.split(';') {
                        match CookieRs::parse(cookie_str.trim()) {
                            Ok(cookie) => cookies.push(cookie),
                            Err(_) => return None
                        }
                    }
                },
                None => return None
            };
        }

        if !cookies.is_empty() {
            Some(Cookie(cookies))
        } else {
            None
        }
    }

    fn fmt_header(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let Cookie(ref value) = *self;
        let last = value.len() - 1;
        for (i, cookie) in value.iter().enumerate() {
            try!(cookie.fmt(fmt));
            if i < last {
                try!("; ".fmt(fmt));
            }
        }
        Ok(())
    }
}

impl Cookie {
    /// This method can be used to crate CookieJar that can be used
    /// to manipulate cookies and create corresponding `SetCookie` header afterwards. 
    #[allow(dead_code)]
    fn to_cookie_jar(&self, key: &[u8]) -> CookieJar {
        let mut jar = CookieJar::new(key);
        let &Cookie(ref cookies) = self;
        for cookie in cookies.iter() {
            jar.add_original(cookie.clone());
        }

        jar   
    }
}

