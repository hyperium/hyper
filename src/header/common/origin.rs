use header::{Header, Raw, Host};
use std::borrow::Cow;
use std::fmt;
use std::str::FromStr;
use header::parsing::from_one_raw_str;

/// The `Origin` header.
///
/// The `Origin` header is a version of the `Referer` header that is used for all HTTP fetches and `POST`s whose CORS flag is set.
/// This header is often used to inform recipients of the security context of where the request was initiated.
///
///
/// Following the spec, https://fetch.spec.whatwg.org/#origin-header, the value of this header is composed of
/// a String (scheme), header::Host (host/port)
///
/// # Examples
/// ```
/// use hyper::header::{Headers, Origin};
///
/// let mut headers = Headers::new();
/// headers.set(
///     Origin::new("http", "hyper.rs", None)
/// );
/// ```
/// ```
/// use hyper::header::{Headers, Origin};
///
/// let mut headers = Headers::new();
/// headers.set(
///     Origin::new("https", "wikipedia.org", Some(443))
/// );
/// ```

#[derive(Clone, Debug)]
pub struct Origin {
    /// The scheme, such as http or https
    scheme: Cow<'static,str>,
    /// The host, such as Host{hostname: "hyper.rs".to_owned(), port: None}
    host: Host,
}

impl Origin {
    /// Creates a new `Origin` header.
    pub fn new<S: Into<Cow<'static,str>>, H: Into<Cow<'static,str>>>(scheme: S, hostname: H, port: Option<u16>) -> Origin{
        Origin {
            scheme: scheme.into(),
            host: Host::new(hostname, port),
        }
    }

    /// The scheme, such as http or https
    /// ```
    /// use hyper::header::Origin;
    /// let origin = Origin::new("https", "foo.com", Some(443));
    /// assert_eq!(origin.scheme(), "https");
    /// ```
    pub fn scheme(&self) -> &str {
        &(self.scheme)
    }

    /// The host, such as Host{hostname: "hyper.rs".to_owned(), port: None}
    /// ```
    /// use hyper::header::{Origin,Host};
    /// let origin = Origin::new("https", "foo.com", Some(443));
    /// assert_eq!(origin.host(), &Host::new("foo.com", Some(443)));
    /// ```
    pub fn host(&self) -> &Host {
        &(self.host)
    }
}

impl Header for Origin {
    fn header_name() -> &'static str {
        static NAME: &'static str = "Origin";
        NAME
    }

    fn parse_header(raw: &Raw) -> ::Result<Origin> {
        from_one_raw_str(raw)
    }

    fn fmt_header(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

static HTTP : &'static str = "http";
static HTTPS : &'static str = "https";

impl FromStr for Origin {
    type Err = ::Error;

    fn from_str(s: &str) -> ::Result<Origin> {
        let idx = match s.find("://") {
            Some(idx) => idx,
            None => return Err(::Error::Header)
        };
        // idx + 3 because that's how long "://" is
        let (scheme, etc) = (&s[..idx], &s[idx + 3..]);
        let host = try!(Host::from_str(etc));
        let scheme = match scheme {
            "http"  => Cow::Borrowed(HTTP),
            "https" => Cow::Borrowed(HTTPS),
            s       => Cow::Owned(s.to_owned())
        };

        Ok(Origin{
            scheme: scheme,
            host: host
        })
    }
}

impl fmt::Display for Origin {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}://{}", self.scheme, self.host)
    }
}

impl PartialEq for Origin {
    fn eq(&self, other: &Origin) -> bool {
        self.scheme == other.scheme && self.host == other.host
    }
}


#[cfg(test)]
mod tests {
    use super::Origin;
    use header::Header;
    use std::borrow::Cow;

    macro_rules! assert_borrowed{
        ($expr : expr) => {
            match $expr {
                Cow::Owned(ref v) => panic!("assertion failed: `{}` owns {:?}", stringify!($expr), v),
                _ => {}
            }
        }
    }

    #[test]
    fn test_origin() {
        let origin : Origin = Header::parse_header(&vec![b"http://foo.com".to_vec()].into()).unwrap();
        assert_eq!(&origin, &Origin::new("http", "foo.com", None));
        assert_borrowed!(origin.scheme);

        let origin : Origin = Header::parse_header(&vec![b"https://foo.com:443".to_vec()].into()).unwrap();
        assert_eq!(&origin, &Origin::new("https", "foo.com", Some(443)));
        assert_borrowed!(origin.scheme);
    }
}

bench_header!(bench, Origin, { vec![b"https://foo.com".to_vec()] });
