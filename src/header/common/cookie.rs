use header::Header;
use std::fmt::{mod, Show};
use std::str::from_utf8;

/// The `Cookie` header
///
/// If the user agent does attach a Cookie header field to an HTTP
/// request, the user agent must send the cookie-string
/// as the value of the header field.
///
/// When the user agent generates an HTTP request, the user agent MUST NOT 
/// attach more than one Cookie header field.
#[deriving(Clone, PartialEq, Show)]
pub struct Cookie(pub Vec<String>);

impl Header for Cookie {
    fn header_name(_: Option<Cookie>) -> &'static str {
        "Cookie"
    }

    fn parse_header(raw: &[Vec<u8>]) -> Option<Cookie> {
        let mut cookies: Vec<String> = vec![];
        for cookies_raw in raw.iter() {
            match from_utf8(cookies_raw.as_slice()) {
                Some(cookies_str) => {
                    for cookie in cookies_str.split(';') {
                        cookies.push(cookie.to_string())
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
        value.connect("; ").fmt(fmt)
    }
}

