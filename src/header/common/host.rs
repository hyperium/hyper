use header::{Header, HeaderFormat};
use std::fmt;
use header::parsing::from_one_raw_str;

/// The `Host` header.
///
/// HTTP/1.1 requires that all requests include a `Host` header, and so hyper
/// client requests add one automatically.
///
/// Currently is just a String, but it should probably become a better type,
/// like url::Host or something.
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
        "Host"
    }

    fn parse_header(raw: &[Vec<u8>]) -> ::Result<Host> {
        from_one_raw_str(raw).and_then(|mut s: String| {
            // FIXME: use rust-url to parse this
            // https://github.com/servo/rust-url/issues/42
            let idx = {
                let slice = &s[..];
                let mut chars = slice.chars();
                chars.next();
                if chars.next().unwrap() == '[' {
                    match slice.rfind(']') {
                        Some(idx) => {
                            if slice.len() > idx + 2 {
                                Some(idx + 1)
                            } else {
                                None
                            }
                        }
                        None => return Err(::Error::Header) // this is a bad ipv6 address...
                    }
                } else {
                    slice.rfind(':')
                }
            };

            let port = match idx {
                Some(idx) => s[idx + 1..].parse().ok(),
                None => None
            };

            match idx {
                Some(idx) => s.truncate(idx),
                None => ()
            }

            Ok(Host {
                hostname: s,
                port: port
            })
        })
    }
}

impl HeaderFormat for Host {
    fn fmt_header(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.port {
            None | Some(80) | Some(443) => f.write_str(&self.hostname[..]),
            Some(port) => write!(f, "{}:{}", self.hostname, port)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Host;
    use header::Header;


    #[test]
    fn test_host() {
        let host = Header::parse_header([b"foo.com".to_vec()].as_ref());
        assert_eq!(host.ok(), Some(Host {
            hostname: "foo.com".to_owned(),
            port: None
        }));


        let host = Header::parse_header([b"foo.com:8080".to_vec()].as_ref());
        assert_eq!(host.ok(), Some(Host {
            hostname: "foo.com".to_owned(),
            port: Some(8080)
        }));
    }
}

bench_header!(bench, Host, { vec![b"foo.com:3000".to_vec()] });
