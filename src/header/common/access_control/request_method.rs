use std::fmt;

use header;
use method::Method;

/// The `Access-Control-Request-Method` request header,
/// part of [CORS](http://www.w3.org/TR/cors/).
///
/// > The `Access-Control-Request-Method` header indicates which method will be
/// > used in the actual request as part of the preflight request.
///
/// Spec: www.w3.org/TR/cors/#access-control-request-method-request-header
#[derive(Clone, PartialEq, Debug)]
pub struct AccessControlRequestMethod(pub Method);

impl header::Header for AccessControlRequestMethod {
    #[inline]
    fn header_name() -> &'static str {
        "Access-Control-Request-Method"
    }

    fn parse_header(raw: &[Vec<u8>]) -> Option<AccessControlRequestMethod> {
        header::parsing::from_one_raw_str(raw).map(AccessControlRequestMethod)
    }
}

impl header::HeaderFormat for AccessControlRequestMethod {
    fn fmt_header(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let AccessControlRequestMethod(ref method) = *self;
        write!(f, "{}", method)
    }
}
