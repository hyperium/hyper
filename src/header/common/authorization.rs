use std::any::Any;
use std::fmt::{self, Display};
use std::str::{FromStr, from_utf8};
use std::ops::{Deref, DerefMut};
use serialize::base64::{ToBase64, FromBase64, Standard, Config, Newline};
use header::{Header, HeaderFormat};

/// `Authorization` header, defined in [RFC7235](https://tools.ietf.org/html/rfc7235#section-4.2)
///
/// The `Authorization` header field allows a user agent to authenticate
/// itself with an origin server -- usually, but not necessarily, after
/// receiving a 401 (Unauthorized) response.  Its value consists of
/// credentials containing the authentication information of the user
/// agent for the realm of the resource being requested.
///
/// # ABNF
/// ```plain
/// Authorization = credentials
/// ```
///
/// # Example values
/// * `Basic QWxhZGRpbjpvcGVuIHNlc2FtZQ==`
/// * `Bearer fpKL54jvWmEGVoRdCNjG`
///
/// # Examples
/// ```
/// use hyper::header::{Headers, Authorization};
///
/// let mut headers = Headers::new();
/// headers.set(Authorization("let me in".to_owned()));
/// ```
/// ```
/// use hyper::header::{Headers, Authorization, Basic};
///
/// let mut headers = Headers::new();
/// headers.set(
///    Authorization(
///        Basic {
///            username: "Aladdin".to_owned(),
///            password: Some("open sesame".to_owned())
///        }
///    )
/// );
/// ```
/// ```
/// use hyper::header::{Headers, Authorization, Bearer};
///
/// let mut headers = Headers::new();
/// headers.set(
///    Authorization(
///        Bearer {
///            token: "QWxhZGRpbjpvcGVuIHNlc2FtZQ".to_owned()
///        }
///    )
/// );
/// ```
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

    fn parse_header(raw: &[Vec<u8>]) -> ::Result<Authorization<S>> {
        if raw.len() != 1 {
            return Err(::Error::Header);
        }
        let header = try!(from_utf8(unsafe { &raw.get_unchecked(0)[..] }));
        return if let Some(scheme) = <S as Scheme>::scheme() {
            if header.starts_with(scheme) && header.len() > scheme.len() + 1 {
                match header[scheme.len() + 1..].parse::<S>().map(Authorization) {
                    Ok(h) => Ok(h),
                    Err(_) => Err(::Error::Header)
                }
            } else {
                Err(::Error::Header)
            }
        } else {
            match header.parse::<S>().map(Authorization) {
                Ok(h) => Ok(h),
                Err(_) => Err(::Error::Header)
            }
        }
    }
}

impl<S: Scheme + Any> HeaderFormat for Authorization<S> where <S as FromStr>::Err: 'static {
    fn fmt_header(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Some(scheme) = <S as Scheme>::scheme() {
            try!(write!(f, "{} ", scheme))
        };
        self.0.fmt_scheme(f)
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
        Display::fmt(self, f)
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
        f.write_str(&text.as_bytes().to_base64(Config {
            char_set: Standard,
            newline: Newline::CRLF,
            pad: true,
            line_length: None
        })[..])
    }
}

impl FromStr for Basic {
    type Err = ::Error;
    fn from_str(s: &str) -> ::Result<Basic> {
        match s.from_base64() {
            Ok(decoded) => match String::from_utf8(decoded) {
                Ok(text) => {
                    let mut parts = &mut text.split(':');
                    let user = match parts.next() {
                        Some(part) => part.to_owned(),
                        None => return Err(::Error::Header)
                    };
                    let password = match parts.next() {
                        Some(part) => Some(part.to_owned()),
                        None => None
                    };
                    Ok(Basic {
                        username: user,
                        password: password
                    })
                },
                Err(e) => {
                    debug!("Basic::from_utf8 error={:?}", e);
                    Err(::Error::Header)
                }
            },
            Err(e) => {
                debug!("Basic::from_base64 error={:?}", e);
                Err(::Error::Header)
            }
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
///Token holder for Bearer Authentication, most often seen with oauth
pub struct Bearer {
	///Actual bearer token as a string
	pub token: String
}

impl Scheme for Bearer {
	fn scheme() -> Option<&'static str> {
		Some("Bearer")
	}

	fn fmt_scheme(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{}", self.token)
	}
}

impl FromStr for Bearer {
	type Err = ::Error;
	fn from_str(s: &str) -> ::Result<Bearer> {
		Ok(Bearer { token: s.to_owned()})
	}
}

#[cfg(test)]
mod tests {
    use super::{Authorization, Basic, Bearer};
    use super::super::super::{Headers, Header};

    #[test]
    fn test_raw_auth() {
        let mut headers = Headers::new();
        headers.set(Authorization("foo bar baz".to_owned()));
        assert_eq!(headers.to_string(), "Authorization: foo bar baz\r\n".to_owned());
    }

    #[test]
    fn test_raw_auth_parse() {
        let header: Authorization<String> = Header::parse_header(
            &[b"foo bar baz".to_vec()]).unwrap();
        assert_eq!(header.0, "foo bar baz");
    }

    #[test]
    fn test_basic_auth() {
        let mut headers = Headers::new();
        headers.set(Authorization(
            Basic { username: "Aladdin".to_owned(), password: Some("open sesame".to_owned()) }));
        assert_eq!(
            headers.to_string(),
            "Authorization: Basic QWxhZGRpbjpvcGVuIHNlc2FtZQ==\r\n".to_owned());
    }

    #[test]
    fn test_basic_auth_no_password() {
        let mut headers = Headers::new();
        headers.set(Authorization(Basic { username: "Aladdin".to_owned(), password: None }));
        assert_eq!(headers.to_string(), "Authorization: Basic QWxhZGRpbjo=\r\n".to_owned());
    }

    #[test]
    fn test_basic_auth_parse() {
        let auth: Authorization<Basic> = Header::parse_header(
            &[b"Basic QWxhZGRpbjpvcGVuIHNlc2FtZQ==".to_vec()]).unwrap();
        assert_eq!(auth.0.username, "Aladdin");
        assert_eq!(auth.0.password, Some("open sesame".to_owned()));
    }

    #[test]
    fn test_basic_auth_parse_no_password() {
        let auth: Authorization<Basic> = Header::parse_header(
            &[b"Basic QWxhZGRpbjo=".to_vec()]).unwrap();
        assert_eq!(auth.0.username, "Aladdin");
        assert_eq!(auth.0.password, Some("".to_owned()));
    }

	#[test]
    fn test_bearer_auth() {
        let mut headers = Headers::new();
        headers.set(Authorization(
            Bearer { token: "fpKL54jvWmEGVoRdCNjG".to_owned() }));
        assert_eq!(
            headers.to_string(),
            "Authorization: Bearer fpKL54jvWmEGVoRdCNjG\r\n".to_owned());
    }

    #[test]
    fn test_bearer_auth_parse() {
        let auth: Authorization<Bearer> = Header::parse_header(
            &[b"Bearer fpKL54jvWmEGVoRdCNjG".to_vec()]).unwrap();
        assert_eq!(auth.0.token, "fpKL54jvWmEGVoRdCNjG");
    }
}

bench_header!(raw, Authorization<String>, { vec![b"foo bar baz".to_vec()] });
bench_header!(basic, Authorization<Basic>, { vec![b"Basic QWxhZGRpbjpuIHNlc2FtZQ==".to_vec()] });
bench_header!(bearer, Authorization<Bearer>, { vec![b"Bearer fpKL54jvWmEGVoRdCNjG".to_vec()] });
