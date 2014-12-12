use std::fmt;
use std::str::{FromStr, from_utf8};
use header::{Header, HeaderFormat};

/// The `WWW-Authenticate` header field.
#[deriving(Clone, PartialEq, Show)]
pub enum WWWAuthenticate {
    /// Basic authentication.
    Basic(BasicParams)
}

/// Parameters for Basic Authentication
#[deriving(Clone, PartialEq, Show)]
pub struct BasicParams {
    /// The authentication realm
    pub realm: String,
}

impl FromStr for BasicParams {
    fn from_str(s: &str) -> Option<BasicParams> {
        const PREFIX: &'static str = "realm=\"";

        if !s.starts_with(PREFIX) {
            debug!("BasicParams::from_str invalid header: {}", s);
            return None
        }

        let s = s.slice_from(PREFIX.len());

        let realm = match s.find('"') {
            Some(i) => s.slice_to(i),
            None => {
                debug!("BasicParams::from_str invalid header: {}", s);
                return None
            }
        };

        Some(BasicParams {
            realm: String::from_str(realm)
        })
    }
}

impl HeaderFormat for BasicParams {
    fn fmt_header(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "Basic realm=\"{}\"", self.realm)
    }
}

impl Header for WWWAuthenticate {
    fn header_name(_: Option<WWWAuthenticate>) -> &'static str {
        "WWW-Authenticate"
    }

    fn parse_header(raw: &[Vec<u8>]) -> Option<WWWAuthenticate> {
        if raw.len() == 1 {
            let header = from_utf8(unsafe { raw[].unsafe_get(0)[] });
            let header = match header {
                Some(s) => s,
                None => {
                    debug!("Invalid utf8 in {} header: {}",
                           "WWW-Authenticate", header);
                    return None
                }
            };

            let scheme = match header.find(' ') {
                Some(i) => header.slice_to(i),
                None => {
                    debug!("Invalid {} header: {}",
                           "WWW-Authenticate", header);
                    return None
                }
            };

            let auth = header.slice_from(scheme.len() + 1);

            match scheme {
                "Basic" => from_str::<BasicParams>(auth).map(|params| {
                    WWWAuthenticate::Basic(params)
                }),
                _ => {
                    debug!("Unhandled WWW-Authenticate scheme ({}): {}",
                           scheme, header);
                    return None
                }
            }
        } else {
            None
        }
    }
}

impl HeaderFormat for WWWAuthenticate {
    fn fmt_header(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &WWWAuthenticate::Basic(ref params) => params.fmt_header(fmt)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::MemReader;
    use super::{WWWAuthenticate, BasicParams};
    use header::Headers;

    const BASIC_RAW: &'static str =
        "WWW-Authenticate: Basic realm=\"Test Realm\"\r\n";

    fn mem(s: &str) -> MemReader {
        MemReader::new(s.as_bytes().to_vec())
    }

    fn _get_basic_struct() -> WWWAuthenticate {
        WWWAuthenticate::Basic(BasicParams { realm: "Test Realm".to_string() })
    }

    #[test]
    fn test_basic_write() {
        let mut headers = Headers::new();
        headers.set(_get_basic_struct());
        assert_eq!(headers.to_string(), BASIC_RAW.to_string());
    }

    #[test]
    fn test_basic_parse() {
        let header_and_blank_str = format!("{}\r\n", BASIC_RAW);
        let header_and_blank = header_and_blank_str.as_slice();
        let headers = Headers::from_raw(&mut mem(header_and_blank)).unwrap();
        assert_eq!(headers.get::<WWWAuthenticate>().unwrap(),
                   &_get_basic_struct());
    }
}

bench_header!(basic, WWWAuthenticate,
              { vec![b"Basic realm=\"Test Realm\"".to_vec()] })
