use header::{Header, HeaderFormat};
use std::fmt::{self};
use std::str::from_utf8;


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
///
/// # Example
/// ```
/// use hyper::header::{Headers, SetCookie};
///
/// let mut headers = Headers::new();
///
/// headers.set(
///     SetCookie(vec![
///         String::from("foo=bar; Path=/path; Domain=example.com")
///     ])
/// );
/// ```
#[derive(Clone, PartialEq, Debug)]
pub struct SetCookie(pub Vec<String>);

__hyper__deref!(SetCookie => Vec<String>);

impl Header for SetCookie {
    fn header_name() -> &'static str {
        "Set-Cookie"
    }

    fn parse_header(raw: &[Vec<u8>]) -> ::Result<SetCookie> {
        let mut set_cookies = Vec::with_capacity(raw.len());
        for set_cookies_raw in raw {
            if let Ok(s) = from_utf8(&set_cookies_raw[..]) {
                set_cookies.push(s.trim().to_owned());
            }
        }

        if !set_cookies.is_empty() {
            Ok(SetCookie(set_cookies))
        } else {
            Err(::Error::Header)
        }
    }

}

impl HeaderFormat for SetCookie {
    fn fmt_header(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.0.len() == 1 {
            write!(f, "{}", &self.0[0])
        } else {
            panic!("SetCookie with multiple cookies cannot be used with fmt_header, must use fmt_multi_header");
        }
    }

    fn fmt_multi_header(&self, f: &mut ::header::MultilineFormatter) -> fmt::Result {
        for cookie in &self.0 {
            try!(f.fmt_line(cookie));
        }
        Ok(())
    }
}

#[test]
fn test_set_cookie_fmt() {
    use ::header::Headers;
    let mut headers = Headers::new();
    headers.set(SetCookie(vec![
        "foo=bar".into(),
        "baz=quux".into(),
    ]));
    assert_eq!(headers.to_string(), "Set-Cookie: foo=bar\r\nSet-Cookie: baz=quux\r\n");
}
