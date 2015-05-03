use header::{Header, HeaderFormat};
use std::fmt::{self, Display};
use std::str::from_utf8;

use cookie::Cookie;
use cookie::CookieJar;

/// `Set-Cookie` header, defined [RFC6265](http://tools.ietf.org/html/rfc6265#section-4.1)
///
/// The Set-Cookie HTTP response header is used to send cookies from the
/// server to the user agent.
///
/// Informally, the Set-Cookie response header contains the header name
/// "Set-Cookie" followed by a ":" and a cookie.  Each cookie begins with
/// a name-value-pair, followed by zero or more attribute-value pairs.
///
/// # ABNF
/// ```plain
///  set-cookie-header = "Set-Cookie:" SP set-cookie-string
/// set-cookie-string = cookie-pair *( ";" SP cookie-av )
/// cookie-pair       = cookie-name "=" cookie-value
/// cookie-name       = token
/// cookie-value      = *cookie-octet / ( DQUOTE *cookie-octet DQUOTE )
/// cookie-octet      = %x21 / %x23-2B / %x2D-3A / %x3C-5B / %x5D-7E
///                       ; US-ASCII characters excluding CTLs,
///                       ; whitespace DQUOTE, comma, semicolon,
///                       ; and backslash
/// token             = <token, defined in [RFC2616], Section 2.2>
///
/// cookie-av         = expires-av / max-age-av / domain-av /
///                    path-av / secure-av / httponly-av /
///                     extension-av
/// expires-av        = "Expires=" sane-cookie-date
/// sane-cookie-date  = <rfc1123-date, defined in [RFC2616], Section 3.3.1>
/// max-age-av        = "Max-Age=" non-zero-digit *DIGIT
///                       ; In practice, both expires-av and max-age-av
///                       ; are limited to dates representable by the
///                       ; user agent.
/// non-zero-digit    = %x31-39
///                       ; digits 1 through 9
/// domain-av         = "Domain=" domain-value
/// domain-value      = <subdomain>
///                       ; defined in [RFC1034], Section 3.5, as
///                       ; enhanced by [RFC1123], Section 2.1
/// path-av           = "Path=" path-value
/// path-value        = <any CHAR except CTLs or ";">
/// secure-av         = "Secure"
/// httponly-av       = "HttpOnly"
/// extension-av      = <any CHAR except CTLs or ";">
/// ```
///
/// # Example values
/// * `SID=31d4d96e407aad42`
/// * `lang=en-US; Expires=Wed, 09 Jun 2021 10:18:14 GMT`
/// * `lang=; Expires=Sun, 06 Nov 1994 08:49:37 GMT`
/// * `lang=en-US; Path=/; Domain=example.com`
#[derive(Clone, PartialEq, Debug)]
pub struct SetCookie(pub Vec<Cookie>);

deref!(SetCookie => Vec<Cookie>);

impl Header for SetCookie {
    fn header_name() -> &'static str {
        "Set-Cookie"
    }

    fn parse_header(raw: &[Vec<u8>]) -> Option<SetCookie> {
        let mut set_cookies = Vec::with_capacity(raw.len());
        for set_cookies_raw in raw {
            if let Ok(s) = from_utf8(&set_cookies_raw[..]) {
                if let Ok(cookie) = s.parse() {
                    set_cookies.push(cookie);
                }
            }
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
            try!(Display::fmt(cookie, f));
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
    let h = Header::parse_header(&[b"foo=bar; HttpOnly".to_vec()][..]);
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

    assert_eq!(&headers.to_string()[..], "Set-Cookie: foo=bar; HttpOnly; Path=/p\r\nSet-Cookie: baz=quux; Path=/\r\n");
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
