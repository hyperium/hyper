use std::fmt::{self, Display};
use std::str;

use header::{Header, Raw};

/// The `Access-Control-Allow-Origin` response header,
/// part of [CORS](http://www.w3.org/TR/cors/#access-control-allow-origin-response-header)
///
/// The `Access-Control-Allow-Origin` header indicates whether a resource
/// can be shared based by returning the value of the Origin request header,
/// `*`, or `null` in the response.
///
/// # ABNF
///
/// ```text
/// Access-Control-Allow-Origin = "Access-Control-Allow-Origin" ":" origin-list-or-null | "*"
/// ```
///
/// # Example values
/// * `null`
/// * `*`
/// * `http://google.com/`
///
/// # Examples
/// ```
/// use hyper::header::{Headers, AccessControlAllowOrigin};
///
/// let mut headers = Headers::new();
/// headers.set(
///     AccessControlAllowOrigin::Any
/// );
/// ```
/// ```
/// use hyper::header::{Headers, AccessControlAllowOrigin};
///
/// let mut headers = Headers::new();
/// headers.set(
///     AccessControlAllowOrigin::Null,
/// );
/// ```
/// ```
/// use hyper::header::{Headers, AccessControlAllowOrigin};
///
/// let mut headers = Headers::new();
/// headers.set(
///     AccessControlAllowOrigin::Value("http://hyper.rs".to_owned())
/// );
/// ```
#[derive(Clone, PartialEq, Debug)]
pub enum AccessControlAllowOrigin {
    /// Allow all origins
    Any,
    /// A hidden origin
    Null,
    /// Allow one particular origin
    Value(String),
}

impl Header for AccessControlAllowOrigin {
    fn header_name() -> &'static str {
        "Access-Control-Allow-Origin"
    }

    fn parse_header(raw: &Raw) -> ::Result<AccessControlAllowOrigin> {
        if let Some(line) = raw.one() {
            Ok(match line {
                b"*" => AccessControlAllowOrigin::Any,
                b"null" => AccessControlAllowOrigin::Null,
                _ => AccessControlAllowOrigin::Value(try!(str::from_utf8(line)).into())
            })
        } else {
            Err(::Error::Header)
        }
    }

    fn fmt_header(&self, f: &mut ::header::Formatter) -> fmt::Result {
        f.fmt_line(self)
    }
}

impl Display for AccessControlAllowOrigin {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        match *self {
            AccessControlAllowOrigin::Any => f.write_str("*"),
            AccessControlAllowOrigin::Null => f.write_str("null"),
            AccessControlAllowOrigin::Value(ref url) => Display::fmt(url, f),
        }
    }
}

#[cfg(test)]
mod test_access_control_allow_origin {
    use header::*;
    use super::AccessControlAllowOrigin as HeaderField;
    test_header!(test1, vec![b"null"]);
    test_header!(test2, vec![b"*"]);
    test_header!(test3, vec![b"http://google.com/"]);
}
