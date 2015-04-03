use std::any::Any;
use std::fmt;
use std::str::{FromStr, from_utf8};
use std::ops::{Deref, DerefMut};
use serialize::base64::{ToBase64, FromBase64, Standard, Config, Newline};
use header::{Header, HeaderFormat};

/// The `Authorization` header field.
#[derive(Clone, PartialEq, Debug)]
pub struct Authorization<S: Scheme>(pub S);

impl<S: Scheme> Deref for Authorization<S> {
    type Target = S;

    fn deref<'a>(&'a self) -> &'a S {
        &self.0
    }
}

impl<S: Scheme> DerefMut for Authorization<S> {
    fn deref_mut<'a>(&'a mut self) -> &'a mut S {
        &mut self.0
    }
}

impl<S: Scheme + Any> Header for Authorization<S> where <S as FromStr>::Err: 'static {
    fn header_name() -> &'static str {
        "Authorization"
    }

    fn parse_header(raw: &[Vec<u8>]) -> Option<Authorization<S>> {
        if raw.len() == 1 {
            match (from_utf8(unsafe { &raw.get_unchecked(0)[..] }), <S as Scheme>::scheme()) {
                (Ok(header), Some(scheme))
                    if header.starts_with(scheme) && header.len() > scheme.len() + 1 => {
                    header[scheme.len() + 1..].parse::<S>().map(Authorization).ok()
                },
                (Ok(header), None) => header.parse::<S>().map(Authorization).ok(),
                _ => None
            }
        } else {
            None
        }
    }
}

impl<S: Scheme + Any> HeaderFormat for Authorization<S> where <S as FromStr>::Err: 'static {
    fn fmt_header(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match <S as Scheme>::scheme() {
            Some(scheme) => try!(write!(fmt, "{} ", scheme)),
            None => ()
        };
        self.0.fmt_scheme(fmt)
    }
}

/// An Authorization scheme to be used in the header.
pub trait Scheme: FromStr + fmt::Debug + Clone + Send + Sync {
    /// An optional Scheme name.
    ///
    /// Will be replaced with an associated constant once available.
    fn scheme() -> Option<&'static str>;
    /// Format the Scheme data into a header value.
    fn fmt_scheme(&self, &mut fmt::Formatter) -> fmt::Result;
}

impl Scheme for String {
    fn scheme() -> Option<&'static str> {
        None
    }

    fn fmt_scheme(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self)
    }
}

/// Credential holder for Basic Authentication
#[derive(Clone, PartialEq, Debug)]
pub struct Basic {
    /// The username as a possibly empty string
    pub username: String,
    /// The password. `None` if the `:` delimiter character was not
    /// part of the parsed input.
    pub password: Option<String>
}

impl Scheme for Basic {
    fn scheme() -> Option<&'static str> {
        Some("Basic")
    }

    fn fmt_scheme(&self, f: &mut fmt::Formatter) -> fmt::Result {
        //FIXME: serialize::base64 could use some Debug implementation, so
        //that we don't have to allocate a new string here just to write it
        //to the formatter.
        let mut text = self.username.clone();
        text.push(':');
        if let Some(ref pass) = self.password {
            text.push_str(&pass[..]);
        }
        write!(f, "{}", text.as_bytes().to_base64(Config {
            char_set: Standard,
            newline: Newline::CRLF,
            pad: true,
            line_length: None
        }))
    }
}

impl FromStr for Basic {
    type Err = ();
    fn from_str(s: &str) -> Result<Basic, ()> {
        match s.from_base64() {
            Ok(decoded) => match String::from_utf8(decoded) {
                Ok(text) => {
                    let mut parts = &mut text.split(':');
                    let user = match parts.next() {
                        Some(part) => part.to_string(),
                        None => return Err(())
                    };
                    let password = match parts.next() {
                        Some(part) => Some(part.to_string()),
                        None => None
                    };
                    Ok(Basic {
                        username: user,
                        password: password
                    })
                },
                Err(e) => {
                    debug!("Basic::from_utf8 error={:?}", e);
                    Err(())
                }
            },
            Err(e) => {
                debug!("Basic::from_base64 error={:?}", e);
                Err(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Authorization, Basic};
    use super::super::super::{Headers, Header};

    #[test]
    fn test_raw_auth() {
        let mut headers = Headers::new();
        headers.set(Authorization("foo bar baz".to_string()));
        assert_eq!(headers.to_string(), "Authorization: foo bar baz\r\n".to_string());
    }

    #[test]
    fn test_raw_auth_parse() {
        let header: Authorization<String> = Header::parse_header(&[b"foo bar baz".to_vec()]).unwrap();
        assert_eq!(header.0, "foo bar baz");
    }

    #[test]
    fn test_basic_auth() {
        let mut headers = Headers::new();
        headers.set(Authorization(Basic { username: "Aladdin".to_string(), password: Some("open sesame".to_string()) }));
        assert_eq!(headers.to_string(), "Authorization: Basic QWxhZGRpbjpvcGVuIHNlc2FtZQ==\r\n".to_string());
    }

    #[test]
    fn test_basic_auth_no_password() {
        let mut headers = Headers::new();
        headers.set(Authorization(Basic { username: "Aladdin".to_string(), password: None }));
        assert_eq!(headers.to_string(), "Authorization: Basic QWxhZGRpbjo=\r\n".to_string());
    }

    #[test]
    fn test_basic_auth_parse() {
        let auth: Authorization<Basic> = Header::parse_header(&[b"Basic QWxhZGRpbjpvcGVuIHNlc2FtZQ==".to_vec()]).unwrap();
        assert_eq!(auth.0.username, "Aladdin");
        assert_eq!(auth.0.password, Some("open sesame".to_string()));
    }

    #[test]
    fn test_basic_auth_parse_no_password() {
        let auth: Authorization<Basic> = Header::parse_header(&[b"Basic QWxhZGRpbjo=".to_vec()]).unwrap();
        assert_eq!(auth.0.username, "Aladdin");
        assert_eq!(auth.0.password, Some("".to_string()));
    }

}

bench_header!(raw, Authorization<String>, { vec![b"foo bar baz".to_vec()] });
bench_header!(basic, Authorization<Basic>, { vec![b"Basic QWxhZGRpbjpvcGVuIHNlc2FtZQ==".to_vec()] });

