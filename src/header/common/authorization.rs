use std::fmt::{mod, Show};
use std::str::{FromStr, from_utf8};
use serialize::base64::{ToBase64, FromBase64, Standard, Config, Newline};
use header::{Header, HeaderFormat};

/// The `Authorization` header field.
#[deriving(Clone, PartialEq, Show)]
pub struct Authorization<S: Scheme>(pub S);

impl<S: Scheme> Deref<S> for Authorization<S> {
    fn deref<'a>(&'a self) -> &'a S {
        &self.0
    }
}

impl<S: Scheme> DerefMut<S> for Authorization<S> {
    fn deref_mut<'a>(&'a mut self) -> &'a mut S {
        &mut self.0
    }
}

impl<S: Scheme> Header for Authorization<S> {
    fn header_name(_: Option<Authorization<S>>) -> &'static str {
        "Authorization"
    }

    fn parse_header(raw: &[Vec<u8>]) -> Option<Authorization<S>> {
        if raw.len() == 1 {
            match (from_utf8(unsafe { raw[].unsafe_get(0)[] }), Scheme::scheme(None::<S>)) {
                (Some(header), Some(scheme))
                    if header.starts_with(scheme) && header.len() > scheme.len() + 1 => {
                    from_str::<S>(header[scheme.len() + 1..]).map(|s| Authorization(s))
                },
                (Some(header), None) => from_str::<S>(header).map(|s| Authorization(s)),
                _ => None
            }
        } else {
            None
        }
    }
}

impl<S: Scheme> HeaderFormat for Authorization<S> {
    fn fmt_header(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match Scheme::scheme(None::<S>) {
            Some(scheme) => try!(write!(fmt, "{} ", scheme)),
            None => ()
        };
        self.0.fmt_scheme(fmt)
    }
}

/// An Authorization scheme to be used in the header.
pub trait Scheme: FromStr + Clone + Send + Sync {
    /// An optional Scheme name.
    ///
    /// For example, `Basic asdf` has the name `Basic`. The Option<Self> is
    /// just a marker that can be removed once UFCS is completed.
    fn scheme(Option<Self>) -> Option<&'static str>;
    /// Format the Scheme data into a header value.
    fn fmt_scheme(&self, &mut fmt::Formatter) -> fmt::Result;
}

impl Scheme for String {
    fn scheme(_: Option<String>) -> Option<&'static str> {
        None
    }

    fn fmt_scheme(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.fmt(f)
    }
}

/// Credential holder for Basic Authentication
#[deriving(Clone, PartialEq, Show)]
pub struct Basic {
    /// The username as a possibly empty string
    pub username: String,
    /// The password. `None` if the `:` delimiter character was not
    /// part of the parsed input.
    pub password: Option<String>
}

impl Scheme for Basic {
    fn scheme(_: Option<Basic>) -> Option<&'static str> {
        Some("Basic")
    }

    fn fmt_scheme(&self, f: &mut fmt::Formatter) -> fmt::Result {
        //FIXME: serialize::base64 could use some Show implementation, so
        //that we don't have to allocate a new string here just to write it
        //to the formatter.
        let mut text = self.username.clone();
        text.push(':');
        if let Some(ref pass) = self.password {
            text.push_str(pass[]);
        }
        text.as_bytes().to_base64(Config {
            char_set: Standard,
            newline: Newline::CRLF,
            pad: true,
            line_length: None
        }).fmt(f)
    }
}

impl FromStr for Basic {
    fn from_str(s: &str) -> Option<Basic> {
        match s.from_base64() {
            Ok(decoded) => match String::from_utf8(decoded) {
                Ok(text) => {
                    let mut parts = text[].split(':');
                    let user = match parts.next() {
                        Some(part) => part.into_string(),
                        None => return None
                    };
                    let password = match parts.next() {
                        Some(part) => Some(part.into_string()),
                        None => None
                    };
                    Some(Basic {
                        username: user,
                        password: password
                    })
                },
                Err(e) => {
                    debug!("Basic::from_utf8 error={}", e);
                    None
                }
            },
            Err(e) => {
                debug!("Basic::from_base64 error={}", e);
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::MemReader;
    use super::{Authorization, Basic};
    use super::super::super::{Headers};

    fn mem(s: &str) -> MemReader {
        MemReader::new(s.as_bytes().to_vec())
    }

    #[test]
    fn test_raw_auth() {
        let mut headers = Headers::new();
        headers.set(Authorization("foo bar baz".into_string()));
        assert_eq!(headers.to_string(), "Authorization: foo bar baz\r\n".to_string());
    }

    #[test]
    fn test_raw_auth_parse() {
        let headers = Headers::from_raw(&mut mem("Authorization: foo bar baz\r\n\r\n")).unwrap();
        assert_eq!(headers.get::<Authorization<String>>().unwrap().0[], "foo bar baz");
    }

    #[test]
    fn test_basic_auth() {
        let mut headers = Headers::new();
        headers.set(Authorization(Basic { username: "Aladdin".into_string(), password: Some("open sesame".into_string()) }));
        assert_eq!(headers.to_string(), "Authorization: Basic QWxhZGRpbjpvcGVuIHNlc2FtZQ==\r\n".into_string());
    }

    #[test]
    fn test_basic_auth_no_password() {
        let mut headers = Headers::new();
        headers.set(Authorization(Basic { username: "Aladdin".to_string(), password: None }));
        assert_eq!(headers.to_string(), "Authorization: Basic QWxhZGRpbjo=\r\n".to_string());
    }

    #[test]
    fn test_basic_auth_parse() {
        let headers = Headers::from_raw(&mut mem("Authorization: Basic QWxhZGRpbjpvcGVuIHNlc2FtZQ==\r\n\r\n")).unwrap();
        let auth = headers.get::<Authorization<Basic>>().unwrap();
        assert_eq!(auth.0.username[], "Aladdin");
        assert_eq!(auth.0.password, Some("open sesame".to_string()));
    }

    #[test]
    fn test_basic_auth_parse_no_password() {
        let headers = Headers::from_raw(&mut mem("Authorization: Basic QWxhZGRpbjo=\r\n\r\n")).unwrap();
        let auth = headers.get::<Authorization<Basic>>().unwrap();
        assert_eq!(auth.0.username.as_slice(), "Aladdin");
        assert_eq!(auth.0.password, Some("".to_string()));
    }

}

bench_header!(raw, Authorization<String>, { vec![b"foo bar baz".to_vec()] });
bench_header!(basic, Authorization<Basic>, { vec![b"Basic QWxhZGRpbjpvcGVuIHNlc2FtZQ==".to_vec()] });

