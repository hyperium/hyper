use std::fmt::{self};

use header;
use header::shared;

/// The `Access-Control-Allow-Headers` response header,
/// part of [CORS](http://www.w3.org/TR/cors/).
///
/// > The `Access-Control-Allow-Headers` header indicates, as part of the
/// > response to a preflight request, which header field names can be used
/// > during the actual request.
///
/// Spec: www.w3.org/TR/cors/#access-control-allow-headers-response-header
///
/// # Examples:
/// ```rust
/// use hyper::header::common;
/// let allowed_headers = common::AccessControlAllowHeaders(vec![
///     "Accept-Encoding".to_string(), "Etag".to_string()]);
/// println!("{}", allowed_headers);
/// ```
#[derive(Clone, PartialEq, Show)]
pub struct AccessControlAllowHeaders(pub Vec<String>);

impl header::Header for AccessControlAllowHeaders {
    #[inline]
    fn header_name(_: Option<AccessControlAllowHeaders>) -> &'static str {
        "Access-Control-Allow-Headers"
    }

    fn parse_header(raw: &[Vec<u8>]) -> Option<AccessControlAllowHeaders> {
        shared::from_comma_delimited(raw).map(AccessControlAllowHeaders)
    }
}

impl header::HeaderFormat for AccessControlAllowHeaders {
    fn fmt_header(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let AccessControlAllowHeaders(ref parts) = *self;
        shared::fmt_comma_delimited(f, parts.as_slice())
    }
}
