use std::fmt::{self, Display};

use header::{Header, HeaderFormat};

/// `Access-Control-Allow-Credentials` header, part of
/// [CORS](http://www.w3.org/TR/cors/#access-control-allow-headers-response-header)
///
/// The Access-Control-Allow-Credentials HTTP response header indicates whether the
/// response to request can be exposed when the credentials flag is true. When part
/// of the response to an preflight request it indicates that the actual request can
/// be made with credentials. The Access-Control-Allow-Credentials HTTP header must
/// match the following ABNF:
///
/// # ABNF
/// ```plain
/// Access-Control-Allow-Credentials: "Access-Control-Allow-Credentials" ":" "true"
/// ```
///
/// Since there is only one acceptable field value, the header struct does not accept
/// any values at all. Setting an empty `AccessControlAllowCredentials` header is
/// sufficient. See the examples below.
///
/// # Example values
/// * "true"
///
/// # Examples
/// ```
/// # extern crate hyper;
/// # fn main() {
///
/// use hyper::header::{Headers, AccessControlAllowCredentials};
///
/// let mut headers = Headers::new();
/// headers.set(AccessControlAllowCredentials);
/// # }
/// ```
#[derive(Clone, PartialEq, Debug)]
pub struct AccessControlAllowCredentials;

impl Header for AccessControlAllowCredentials {
    fn header_name() -> &'static str {
        "Access-Control-Allow-Credentials"
    }

    fn parse_header(raw: &[Vec<u8>]) -> ::Result<AccessControlAllowCredentials> {
        if raw.len() == 1 && unsafe { raw.get_unchecked(0) } == b"true" {
            return Ok(AccessControlAllowCredentials);
        }
        Err(::Error::Header)
    }
}

impl HeaderFormat for AccessControlAllowCredentials {
    fn fmt_header(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("true")
    }
}

impl Display for AccessControlAllowCredentials {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        self.fmt_header(f)
    }
}

#[cfg(test)]
mod test_access_control_allow_credentials {
    use std::str;
    use header::*;
    use super::AccessControlAllowCredentials as HeaderField;
    test_header!(test1, vec![b"true"], Some(HeaderField));
    test_header!(test2, vec![b"false"], None);
    test_header!(test3, vec![b"true", b"true"], None);
    test_header!(test4, vec!["\u{645}\u{631}\u{62d}\u{628}\u{627}".as_bytes()], None);
}