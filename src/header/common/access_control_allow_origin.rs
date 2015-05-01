use std::fmt;
use std::str;

use url::Url;
use header;

/// The `Access-Control-Allow-Origin` response header,
/// part of [CORS](www.w3.org/TR/cors/#access-control-allow-origin-response-header)
///
/// The `Access-Control-Allow-Origin` header indicates whether a resource
/// can be shared based by returning the value of the Origin request header,
/// "*", or "null" in the response.
///
/// # ABNF
/// ```plain
/// Access-Control-Allow-Origin = "Access-Control-Allow-Origin" ":" origin-list-or-null | "*"
/// ```
// FIXME: The documentation says differently (missing "null" value, "*" not used in practice,
// orgin list no list but single value)
#[derive(Clone, PartialEq, Debug)]
pub enum AccessControlAllowOrigin {
    /// Allow all origins
    Any,
    /// Allow one particular origin
    Value(Url),
}

impl header::Header for AccessControlAllowOrigin {
    fn header_name() -> &'static str {
        "Access-Control-Allow-Origin"
    }

    fn parse_header(raw: &[Vec<u8>]) -> Option<AccessControlAllowOrigin> {
        if raw.len() == 1 {
            match str::from_utf8(unsafe { &raw.get_unchecked(0)[..] }) {
                Ok(s) => {
                    if s == "*" {
                        Some(AccessControlAllowOrigin::Any)
                    } else {
                        Url::parse(s).ok().map(
                            |url| AccessControlAllowOrigin::Value(url))
                    }
                },
                _ => return None,
            }
        } else {
            return None;
        }
    }
}

impl header::HeaderFormat for AccessControlAllowOrigin {
    fn fmt_header(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            AccessControlAllowOrigin::Any => f.write_str("*"),
            AccessControlAllowOrigin::Value(ref url) => fmt::Display::fmt(url, f)
        }
    }
}
