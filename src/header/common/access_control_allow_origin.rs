use std::fmt::{self, Display};
use std::str;

use url::Url;
use header::{Header, HeaderFormat};

/// The `Access-Control-Allow-Origin` response header,
/// part of [CORS](http://www.w3.org/TR/cors/#access-control-allow-origin-response-header)
///
/// The `Access-Control-Allow-Origin` header indicates whether a resource
/// can be shared based by returning the value of the Origin request header,
/// "*", or "null" in the response.
///
/// # ABNF
/// ```plain
/// Access-Control-Allow-Origin = "Access-Control-Allow-Origin" ":" origin-list-or-null | "*"
/// ```
///
/// # Example values
/// * `null`
/// * `*`
/// * `http://google.com/`
#[derive(Clone, PartialEq, Debug)]
pub enum AccessControlAllowOrigin {
    /// Allow all origins
    Any,
    /// A hidden origin
    Null,
    /// Allow one particular origin
    Value(Url),
}

impl Header for AccessControlAllowOrigin {
    fn header_name() -> &'static str {
        "Access-Control-Allow-Origin"
    }

    fn parse_header(raw: &[Vec<u8>]) -> Option<AccessControlAllowOrigin> {
        if raw.len() == 1 {
            match unsafe { &raw.get_unchecked(0)[..] } {
                b"*" => Some(AccessControlAllowOrigin::Any),
                b"null" => Some(AccessControlAllowOrigin::Null),
                r => if let Ok(s) = str::from_utf8(r) {
                    Url::parse(s).ok().map(AccessControlAllowOrigin::Value)
                } else { None }
            }
        } else { None }
    }
}

impl HeaderFormat for AccessControlAllowOrigin {
    fn fmt_header(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            AccessControlAllowOrigin::Any => f.write_str("*"),
            AccessControlAllowOrigin::Null => f.write_str("null"),
            AccessControlAllowOrigin::Value(ref url) => Display::fmt(url, f),
        }
    }
}

impl Display for AccessControlAllowOrigin {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        self.fmt_header(f)
    }
}

#[cfg(test)]
mod test_access_control_allow_orgin {
    use header::*;
    use super::AccessControlAllowOrigin as HeaderField;
    test_header!(test1, vec![b"null"]);
    test_header!(test2, vec![b"*"]);
    test_header!(test3, vec![b"http://google.com/"]);
}
