use header::{Header, HeaderFormat};
use std::fmt::{self, Display};
use std::str::from_utf8;

/// `Cookie` header, defined in [RFC6265](http://tools.ietf.org/html/rfc6265#section-5.4)
///
/// If the user agent does attach a Cookie header field to an HTTP
/// request, the user agent must send the cookie-string
/// as the value of the header field.
///
/// When the user agent generates an HTTP request, the user agent MUST NOT
/// attach more than one Cookie header field.
///
/// # Example values
/// * `SID=31d4d96e407aad42`
/// * `SID=31d4d96e407aad42; lang=en-US`
///
/// # Example
/// ```
/// use hyper::header::{Headers, Cookie};
///
/// let mut headers = Headers::new();
///
/// headers.set(
///    Cookie(vec![
///        String::from("foo=bar")
///    ])
/// );
/// ```
#[derive(Clone, PartialEq, Debug)]
pub struct Cookie(pub Vec<String>);

__hyper__deref!(Cookie => Vec<String>);

impl Header for Cookie {
    fn header_name() -> &'static str {
        "Cookie"
    }

    fn parse_header(raw: &[Vec<u8>]) -> ::Result<Cookie> {
        let mut cookies = Vec::with_capacity(raw.len());
        for cookies_raw in raw.iter() {
            let cookies_str = try!(from_utf8(&cookies_raw[..]));
            for cookie_str in cookies_str.split(';') {
                cookies.push(cookie_str.trim().to_owned())
            }
        }

        if !cookies.is_empty() {
            Ok(Cookie(cookies))
        } else {
            Err(::Error::Header)
        }
    }
}

impl HeaderFormat for Cookie {
    fn fmt_header(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let cookies = &self.0;
        for (i, cookie) in cookies.iter().enumerate() {
            if i != 0 {
                try!(f.write_str("; "));
            }
            try!(Display::fmt(&cookie, f));
        }
        Ok(())
    }
}

bench_header!(bench, Cookie, { vec![b"foo=bar; baz=quux".to_vec()] });
