use header::Header;
use std::fmt::{mod, Show};
use std::str::from_utf8;
use std::from_str::FromStr;

#[cfg(feature = "cookie_rs")]
use cookie::Cookie as CookieRs;
#[cfg(feature = "cookie_rs")]
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
pub struct TypedCookie<T>(pub Vec<T>);

impl<T: FromStr + Show + Clone + Send + Sync> Header for TypedCookie<T> {
    fn header_name(_: Option<TypedCookie<T>>) -> &'static str {
        "Cookie"
    }

    fn parse_header(raw: &[Vec<u8>]) -> Option<TypedCookie<T>> {
        let mut cookies: Vec<T> = vec![];
        for cookies_raw in raw.iter() {
            match from_utf8(cookies_raw.as_slice()) {
                Some(cookies_str) => {
                    for cookie_str in cookies_str.split(';') {
                        match from_str(cookie_str.trim()) {
                            Some(cookie) => cookies.push(cookie),
                            None => return None
                        }
                    }
                },
                None => return None
            };
        }

        if !cookies.is_empty() {
            Some(TypedCookie(cookies))
        } else {
            None
        }
    }

    fn fmt_header(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let TypedCookie(ref value) = *self;
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

#[cfg(not(feature = "cookie_rs"))]
pub type Cookie = TypedCookie<String>;

#[cfg(feature = "cookie_rs")]
pub type Cookie = TypedCookie<CookieRs>;

#[cfg(feature = "cookie_rs")]
impl Cookie {
    /// This method can be used to crate CookieJar that can be used
    /// to manipulate cookies and create corresponding `SetCookie` header afterwards. 
    #[allow(dead_code)]
    fn to_cookie_jar(&self, key: &[u8]) -> CookieJar {
        let mut jar = CookieJar::new(key);
        let &TypedCookie(ref cookies) = self;
        for cookie in cookies.iter() {
            jar.add_original((*cookie).clone());
        }

        jar   
    }
}

