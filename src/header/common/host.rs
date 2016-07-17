use header::{Header, Raw};
use std::fmt;
use std::str::FromStr;
use header::parsing::from_one_raw_str;
use url::idna::domain_to_unicode;

/// The `Host` header.
///
/// HTTP/1.1 requires that all requests include a `Host` header, and so hyper
/// client requests add one automatically.
///
/// Currently is just a String, but it should probably become a better type,
/// like `url::Host` or something.
///
/// # Examples
/// ```
/// use hyper::header::{Headers, Host};
///
/// let mut headers = Headers::new();
/// headers.set(
///     Host{
///         hostname: "hyper.rs".to_owned(),
///         port: None,
///     }
/// );
/// ```
/// ```
/// use hyper::header::{Headers, Host};
///
/// let mut headers = Headers::new();
/// headers.set(
///     Host{
///         hostname: "hyper.rs".to_owned(),
///         port: Some(8080),
///     }
/// );
/// ```
#[derive(Clone, PartialEq, Debug)]
pub struct Host {
    /// The hostname, such a example.domain.
    pub hostname: String,
    /// An optional port number.
    pub port: Option<u16>
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
        let (host_port, res) = domain_to_unicode(s);
        if res.is_err() {
            return Err(::Error::Header)
        }
        let idx = host_port.rfind(':');
        let port = idx.and_then(
            |idx| s[idx + 1..].parse().ok()
        );
        let hostname = match idx {
            None => host_port,
            Some(idx) => host_port[..idx].to_owned()
        };
        Ok(Host {
            hostname: hostname,
            port: port
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
        assert_eq!(host.ok(), Some(Host {
            hostname: "foo.com".to_owned(),
            port: None
        }));


        let host = Header::parse_header(&vec![b"foo.com:8080".to_vec()].into());
        assert_eq!(host.ok(), Some(Host {
            hostname: "foo.com".to_owned(),
            port: Some(8080)
        }));
    }
}

bench_header!(bench, Host, { vec![b"foo.com:3000".to_vec()] });
