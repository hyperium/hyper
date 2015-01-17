use std::fmt::{self};

use header;
use method;

/// The `Access-Control-Allow-Methods` response header,
/// part of [CORS](http://www.w3.org/TR/cors/).
///
/// > The `Access-Control-Allow-Methods` header indicates, as part of the
/// > response to a preflight request, which methods can be used during the
/// > actual request.
///
/// Spec: www.w3.org/TR/cors/#access-control-allow-methods-response-header
#[derive(Clone, PartialEq, Show)]
pub struct AccessControlAllowMethods(pub Vec<method::Method>);

impl header::Header for AccessControlAllowMethods {
    #[inline]
    fn header_name(_: Option<AccessControlAllowMethods>) -> &'static str {
        "Access-Control-Allow-Methods"
    }

    fn parse_header(raw: &[Vec<u8>]) -> Option<AccessControlAllowMethods> {
        header::parsing::from_comma_delimited(raw).map(AccessControlAllowMethods)
    }
}

impl header::HeaderFormat for AccessControlAllowMethods {
    fn fmt_header(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let AccessControlAllowMethods(ref parts) = *self;
        header::parsing::fmt_comma_delimited(f, parts.as_slice())
    }
}
