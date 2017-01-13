use std::borrow::Cow;
use std::fmt;
use std::str::FromStr;

use header::{Header, Raw};
use header::parsing::from_one_raw_str;

/// The `Host` header.
///
/// HTTP/1.1 requires that all requests include a `Host` header, and so hyper
/// client requests add one automatically.
///
/// # Examples
/// ```
/// use hyper::header::{Headers, Host};
///
/// let mut headers = Headers::new();
/// headers.set(
///     Host::new("hyper.rs", None)
/// );
/// ```
/// ```
/// use hyper::header::{Headers, Host};
///
/// let mut headers = Headers::new();
/// headers.set(
///     Host::new("hyper.rs", 8080)
/// );
/// ```
#[derive(Clone, PartialEq, Debug)]
pub struct Host {
    hostname: Cow<'static, str>,
    port: Option<u16>
}

impl Host {
    /// Create a `Host` header, providing the hostname and optional port.
    pub fn new<H, P>(hostname: H, port: P) -> Host
    where H: Into<Cow<'static, str>>,
          P: Into<Option<u16>>
    {
        Host {
            hostname: hostname.into(),
            port: port.into(),
        }
    }

    /// Get the hostname, such as example.domain.
    pub fn hostname(&self) -> &str {
        self.hostname.as_ref()
    }

    /// Get the optional port number.
    pub fn port(&self) -> Option<u16> {
        self.port
    }
}

impl Header for Host {
    fn header_name() -> &'static str {
        static NAME: &'static str = "Host";
        NAME
    }

    fn parse_header(raw: &Raw) -> ::Result<Host> {
       from_one_raw_str(raw)
    }

    fn fmt_header(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.port {
            None | Some(80) | Some(443) => f.write_str(&self.hostname[..]),
            Some(port) => write!(f, "{}:{}", self.hostname, port)
        }
    }
}

impl fmt::Display for Host {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.fmt_header(f)
    }
}

impl FromStr for Host {
    type Err = ::Error;

    fn from_str(s: &str) -> ::Result<Host> {
        let idx = s.rfind(':');
        let port = idx.and_then(
            |idx| s[idx + 1..].parse().ok()
        );
        let hostname = match port {
            None => s,
            Some(_) => &s[..idx.unwrap()]
        };

        Ok(Host {
            hostname: hostname.to_owned().into(),
            port: port,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::Host;
    use header::Header;


    #[test]
    fn test_host() {
        let host = Header::parse_header(&vec![b"foo.com".to_vec()].into());
        assert_eq!(host.ok(), Some(Host::new("foo.com", None)));


        let host = Header::parse_header(&vec![b"foo.com:8080".to_vec()].into());
        assert_eq!(host.ok(), Some(Host::new("foo.com", 8080)));

        let host = Header::parse_header(&vec![b"foo.com".to_vec()].into());
        assert_eq!(host.ok(), Some(Host::new("foo.com", None)));

        let host = Header::parse_header(&vec![b"[::1]:8080".to_vec()].into());
        assert_eq!(host.ok(), Some(Host::new("[::1]", 8080)));

        let host = Header::parse_header(&vec![b"[::1]".to_vec()].into());
        assert_eq!(host.ok(), Some(Host::new("[::1]", None)));
    }
}

bench_header!(bench, Host, { vec![b"foo.com:3000".to_vec()] });
