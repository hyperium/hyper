use std::fmt;
use std::str::{self, FromStr};

use unicase;

use header::{Header, Raw, parsing};

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
/// * `max-age=31536000 ; includeSubDomains ; preload`
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
///     {
///         let mut StrictTransport = StrictTransportSecurity::new();
///         StrictTransport.set_max_age(StrictTransportSecurity::preload_min_age());
///         StrictTransport.set_include_subdomains(true);
///         StrictTransport.set_preload(true);
///         StrictTransport
///     }
/// );
/// # }
/// ```
#[derive(Clone, PartialEq, Debug)]
pub struct StrictTransportSecurity {
    /// Signals the UA that the HSTS Policy applies to this HSTS Host as well as
    /// any subdomains of the host's domain name.
    include_subdomains: bool,

    /// Specifies the number of seconds, after the reception of the STS header
    /// field, during which the UA regards the host (from whom the message was
    /// received) as a Known HSTS Host.
    max_age: u64,

    /// Indicates that the server wants to be included into the HSTS preload list,
    /// according to [HSTS preload site](https://hstspreload.org/)
    /// this value should be set to true only if the site:
    /// 
    /// * Serves a valid certificate.
    /// * Redirects all of the traffic on port 80 to port 443.
    /// * Serves all of its subdomains via HTTPS.
    /// * Sets in the HSTS headers the parameter includeSubDomains and has max-age
    ///   bigger than 31536000 seconds (one year).
    /// * Any redirect will still be served with a HSTS header
    preload: bool
}

/// This value indicates the minimum value of the max_age
/// that can be used together with preload as per
/// [HSTS preload](https://hstspreload.org/) guidelines
const PRELOAD_MIN_AGE: u64 = 31536000;

impl StrictTransportSecurity {
    /// Create an STS header with a expiration date
    /// of 5 minutes with no subdomains or preload
    pub fn new() -> StrictTransportSecurity {
        StrictTransportSecurity {
            max_age: 300,
            include_subdomains: false,
            preload: false
        }
    }

    /// Returns the inserted max age
    pub fn max_age(&self) -> u64 { self.max_age }

    /// Sets the max age
    pub fn set_max_age(&mut self, max_age: u64) {
        self.max_age = max_age;
    }

    /// Returns if the header applies HSTS also to subdomains
    pub fn include_subdomains(&self) -> bool { self.include_subdomains }

    /// Sets if the HSTS header applies also to subdomains
    pub fn set_include_subdomains(&mut self, include_subdomains: bool) {
        self.include_subdomains = include_subdomains;
    }

    /// Returns if the header applies HSTS preload
    pub fn preload(&self) -> bool { self.preload }

    /// Sets if the HSTS header applies HSTS preload.
    pub fn set_preload(&mut self, preload: bool) {
        self.preload = preload;
    }

    /// Returns the minimum value that
    /// the max_age field should have in order to
    /// use preload as per [HSTS preload](https://hstspreload.org/)
    /// guidelines
    pub fn preload_min_age() -> u64 { PRELOAD_MIN_AGE }

    /// Returns if the current header
    /// is capable of being included into
    /// [HSTS preload lists](https://hstspreload.org/)
    pub fn preload_list_capable(&self) -> bool {
        if self.include_subdomains && self.max_age >= PRELOAD_MIN_AGE {
            true
        } else {
            false
        }
    }
}

enum Directive {
    MaxAge(u64),
    IncludeSubdomains,
    Preload,
    Unknown
}

impl FromStr for StrictTransportSecurity {
    type Err = ::Error;

    fn from_str(s: &str) -> ::Result<StrictTransportSecurity> {
        s.split(';')
            .map(str::trim)
            .map(|sub| match sub {
                sub if unicase::eq_ascii(sub, "includeSubDomains") => 
                    Ok(Directive::IncludeSubdomains),
                sub if unicase::eq_ascii(sub, "preload") => 
                    Ok(Directive::Preload),
                sub => {
                    let mut sub = sub.splitn(2, '=');
                    match (sub.next(), sub.next()) {
                        (Some(left), Some(right))
                        if unicase::eq_ascii(left.trim(), "max-age") => {
                            right
                                .trim()
                                .trim_matches('"')
                                .parse()
                                .map(Directive::MaxAge)
                        },
                        _ => Ok(Directive::Unknown)
                    }
                }
            })
            .fold(Ok((None, None, None)), |res, dir| match (res, dir) {
                ( Ok((None, subd, prel)), Ok(Directive::MaxAge(age))) => Ok((Some(age),subd,prel)),
                ( Ok((mage, None, prel)), Ok(Directive::IncludeSubdomains)) => Ok((mage,Some(true),prel)),
                ( Ok((mage, subd, None)), Ok(Directive::Preload)) => Ok((mage,subd,Some(true))),
                ( Ok((Some(_), _, _)), Ok(Directive::MaxAge(_))) |
                ( Ok((_, Some(_), _)), Ok(Directive::IncludeSubdomains)) |
                ( Ok((_, _, Some(_))), Ok(Directive::Preload)) |
                ( _, Err(_) ) => Err(::Error::Header),
                ( res, _ ) => res
            })
            .and_then(|res| match res {
                (Some(age), sub, pre) => Ok(StrictTransportSecurity{
                    max_age: age,
                    include_subdomains: sub.is_some(),
                    preload: pre.is_some()
                }),
                _ => Err(::Error::Header)
            })
    }
}

impl Header for StrictTransportSecurity {
    fn header_name() -> &'static str {
        static NAME: &'static str = "Strict-Transport-Security";
        NAME
    }

    fn parse_header(raw: &Raw) -> ::Result<StrictTransportSecurity> {
        parsing::from_one_raw_str(raw)
    }

    fn fmt_header(&self, f: &mut ::header::Formatter) -> fmt::Result {
        f.fmt_line(self)
    }
}

impl fmt::Display for StrictTransportSecurity {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.include_subdomains {
            if self.preload {
                write!(f, "max-age={}; includeSubdomains; preload", self.max_age)
            } else {
                write!(f, "max-age={}; includeSubdomains", self.max_age)
            }
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
        let h = Header::parse_header(&"max-age=31536000".into());
        assert_eq!(h.ok(), Some(StrictTransportSecurity { include_subdomains: false, max_age: 31536000u64, preload: false }));
    }

    #[test]
    fn test_parse_max_age_no_value() {
        let h: ::Result<StrictTransportSecurity> = Header::parse_header(&"max-age".into());
        assert!(h.is_err());
    }

    #[test]
    fn test_parse_quoted_max_age() {
        let h = Header::parse_header(&"max-age=\"31536000\"".into());
        assert_eq!(h.ok(), Some(StrictTransportSecurity { include_subdomains: false, max_age: 31536000u64, preload: false }));
    }

    #[test]
    fn test_parse_spaces_max_age() {
        let h = Header::parse_header(&"max-age = 31536000".into());
        assert_eq!(h.ok(), Some(StrictTransportSecurity { include_subdomains: false, max_age: 31536000u64, preload: false }));
    }

    #[test]
    fn test_parse_include_subdomains() {
        let h = Header::parse_header(&"max-age=15768000 ; includeSubDomains".into());
        assert_eq!(h.ok(), Some(StrictTransportSecurity { include_subdomains: true, max_age: 15768000u64, preload: false }));
    }

    #[test]
    fn test_parse_preload() {
        let h = Header::parse_header(&"max-age=31536000 ; includeSubDomains; preload".into());
        assert_eq!(h.ok(), Some(StrictTransportSecurity { include_subdomains: true, max_age: 31536000, preload: true }));
    }


    #[test]
    fn test_preload_setter() {
        let h = {
            let mut h =StrictTransportSecurity::new();
            let min_age = StrictTransportSecurity::preload_min_age();
            h.set_max_age(min_age);
            h.set_include_subdomains(true);
            h.set_preload(true);
            h
        };
        assert_eq!(h, StrictTransportSecurity { include_subdomains: true, max_age: StrictTransportSecurity::preload_min_age(), preload: true });
    }

    #[test]
    fn test_preload_list() {
        let valid_preload = StrictTransportSecurity { include_subdomains: true, max_age: StrictTransportSecurity::preload_min_age(), preload: true };
        let invalid_preload = StrictTransportSecurity { include_subdomains: false, max_age: StrictTransportSecurity::preload_min_age(), preload: true };
        assert!(valid_preload.preload_list_capable());
        assert!( ! invalid_preload.preload_list_capable());
    }

    #[test]
    fn test_parse_no_max_age() {
        let h: ::Result<StrictTransportSecurity> = Header::parse_header(&"includeSubDomains".into());
        assert!(h.is_err());
    }

    #[test]
    fn test_parse_max_age_nan() {
        let h: ::Result<StrictTransportSecurity> = Header::parse_header(&"max-age = derp".into());
        assert!(h.is_err());
    }

    #[test]
    fn test_parse_duplicate_directives() {
        assert!(StrictTransportSecurity::parse_header(&"max-age=100; max-age=5; max-age=0".into()).is_err());
    }
}

bench_header!(bench, StrictTransportSecurity, { vec![b"max-age=63072000 ; includeSubDomains ; preload".to_vec()] });