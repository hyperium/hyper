use std::fmt;
use std::str::{self, FromStr};

use unicase::UniCase;

use header::{Header, HeaderFormat, parsing};

/// `StrictTransportSecurity` header, defined in [RFC6797](https://tools.ietf.org/html/rfc6797)
///
/// This specification defines a mechanism enabling web sites to declare
/// themselves accessible only via secure connections and/or for users to be
/// able to direct their user agent(s) to interact with given sites only over
/// secure connections.  This overall policy is referred to as HTTP Strict
/// Transport Security (HSTS).  The policy is declared by web sites via the
/// Strict-Transport-Security HTTP response header field and/or by other means,
/// such as user agent configuration, for example.
///
/// # ABNF
///
/// ```plain
///      [ directive ]  *( ";" [ directive ] )
///
///      directive                 = directive-name [ "=" directive-value ]
///      directive-name            = token
///      directive-value           = token | quoted-string
///
/// ```
///
/// # Example values
/// * `max-age=31536000`
/// * `max-age=15768000 ; includeSubDomains`
///
/// # Example
/// ```
/// # extern crate hyper;
/// # fn main() {
/// use hyper::header::{Headers, StrictTransportSecurity};
///
/// let mut headers = Headers::new();
///
/// headers.set(
///    StrictTransportSecurity::including_subdomains(31536000u64)
/// );
/// # }
/// ```
#[derive(Clone, PartialEq, Debug)]
pub struct StrictTransportSecurity {
    /// Signals the UA that the HSTS Policy applies to this HSTS Host as well as
    /// any subdomains of the host's domain name.
    pub include_subdomains: bool,

    /// Specifies the number of seconds, after the reception of the STS header
    /// field, during which the UA regards the host (from whom the message was
    /// received) as a Known HSTS Host.
    pub max_age: u64
}

impl StrictTransportSecurity {
    /// Create an STS header that includes subdomains
    pub fn including_subdomains(max_age: u64) -> StrictTransportSecurity {
        StrictTransportSecurity {
            max_age: max_age,
            include_subdomains: true
        }
    }

    /// Create an STS header that excludes subdomains
    pub fn excluding_subdomains(max_age: u64) -> StrictTransportSecurity {
        StrictTransportSecurity {
            max_age: max_age,
            include_subdomains: false
        }
    }
}

enum Directive {
    MaxAge(u64),
    IncludeSubdomains,
    Unknown
}

impl FromStr for StrictTransportSecurity {
    type Err = ::Error;

    fn from_str(s: &str) -> ::Result<StrictTransportSecurity> {
        s.split(';')
            .map(str::trim)
            .map(|sub| if UniCase(sub) == UniCase("includeSubdomains") {
                Ok(Directive::IncludeSubdomains)
            } else {
                let mut sub = sub.splitn(2, '=');
                match (sub.next(), sub.next()) {
                    (Some(left), Some(right))
                    if UniCase(left.trim()) == UniCase("max-age") => {
                        right
                            .trim()
                            .trim_matches('"')
                            .parse()
                            .map(Directive::MaxAge)
                    },
                    _ => Ok(Directive::Unknown)
                }
            })
            .fold(Ok((None, None)), |res, dir| match (res, dir) {
                (Ok((None, sub)), Ok(Directive::MaxAge(age))) => Ok((Some(age), sub)),
                (Ok((age, None)), Ok(Directive::IncludeSubdomains)) => Ok((age, Some(()))),
                (Ok((Some(_), _)), Ok(Directive::MaxAge(_))) => Err(::Error::Header),
                (Ok((_, Some(_))), Ok(Directive::IncludeSubdomains)) => Err(::Error::Header),
                (_, Err(_)) => Err(::Error::Header),
                (res, _) => res
            })
            .and_then(|res| match res {
                (Some(age), sub) => Ok(StrictTransportSecurity {
                    max_age: age,
                    include_subdomains: sub.is_some()
                }),
                _ => Err(::Error::Header)
            })
    }
}

impl Header for StrictTransportSecurity {
    fn header_name() -> &'static str {
        "Strict-Transport-Security"
    }

    fn parse_header(raw: &[Vec<u8>]) -> ::Result<StrictTransportSecurity> {
        parsing::from_one_raw_str(raw)
    }
}

impl HeaderFormat for StrictTransportSecurity {
    fn fmt_header(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.include_subdomains {
            write!(f, "max-age={}; includeSubdomains", self.max_age)
        } else {
            write!(f, "max-age={}", self.max_age)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::StrictTransportSecurity;
    use header::Header;

    #[test]
    fn test_parse_max_age() {
        let h = Header::parse_header(&[b"max-age=31536000".to_vec()][..]);
        assert_eq!(h.ok(), Some(StrictTransportSecurity { include_subdomains: false, max_age: 31536000u64 }));
    }

    #[test]
    fn test_parse_max_age_no_value() {
        let h: ::Result<StrictTransportSecurity> = Header::parse_header(&[b"max-age".to_vec()][..]);
        assert!(h.is_err());
    }

    #[test]
    fn test_parse_quoted_max_age() {
        let h = Header::parse_header(&[b"max-age=\"31536000\"".to_vec()][..]);
        assert_eq!(h.ok(), Some(StrictTransportSecurity { include_subdomains: false, max_age: 31536000u64 }));
    }

    #[test]
    fn test_parse_spaces_max_age() {
        let h = Header::parse_header(&[b"max-age = 31536000".to_vec()][..]);
        assert_eq!(h.ok(), Some(StrictTransportSecurity { include_subdomains: false, max_age: 31536000u64 }));
    }

    #[test]
    fn test_parse_include_subdomains() {
        let h = Header::parse_header(&[b"max-age=15768000 ; includeSubDomains".to_vec()][..]);
        assert_eq!(h.ok(), Some(StrictTransportSecurity { include_subdomains: true, max_age: 15768000u64 }));
    }

    #[test]
    fn test_parse_no_max_age() {
        let h: ::Result<StrictTransportSecurity> = Header::parse_header(&[b"includeSubDomains".to_vec()][..]);
        assert!(h.is_err());
    }

    #[test]
    fn test_parse_max_age_nan() {
        let h: ::Result<StrictTransportSecurity> = Header::parse_header(&[b"max-age = derp".to_vec()][..]);
        assert!(h.is_err());
    }

    #[test]
    fn test_parse_duplicate_directives() {
        assert!(StrictTransportSecurity::parse_header(&[b"max-age=100; max-age=5; max-age=0".to_vec()][..]).is_err());
    }
}

bench_header!(bench, StrictTransportSecurity, { vec![b"max-age=15768000 ; includeSubDomains".to_vec()] });
