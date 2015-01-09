use std::fmt::{self};

use header;
use header::shared;

/// The `Access-Control-Request-Headers` request header,
/// part of [CORS](http://www.w3.org/TR/cors/).
///
/// > The `Access-Control-Request-Headers` header indicates which headers will
/// > be used in the actual request as part of the preflight request.
///
/// Spec: www.w3.org/TR/cors/#access-control-request-headers-request-header
#[derive(Clone, PartialEq, Show)]
pub struct AccessControlRequestHeaders(pub Vec<String>);

impl header::Header for AccessControlRequestHeaders {
    #[inline]
    fn header_name(_: Option<AccessControlRequestHeaders>) -> &'static str {
        "Access-Control-Request-Headers"
    }

    fn parse_header(raw: &[Vec<u8>]) -> Option<AccessControlRequestHeaders> {
        shared::from_comma_delimited(raw).map(AccessControlRequestHeaders)
    }
}

impl header::HeaderFormat for AccessControlRequestHeaders {
    fn fmt_header(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let AccessControlRequestHeaders(ref parts) = *self;
        shared::fmt_comma_delimited(f, parts.as_slice())
    }
}
