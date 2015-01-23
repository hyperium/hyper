use std::fmt;

use header;

/// The `Access-Control-Max-Age` response header,
/// part of [CORS](http://www.w3.org/TR/cors/).
///
/// > The `Access-Control-Max-Age` header indicates how long the results of a
/// > preflight request can be cached in a preflight result cache.
///
/// Spec: www.w3.org/TR/cors/#access-control-max-age-response-header
#[derive(Clone, Copy, PartialEq, Show)]
pub struct AccessControlMaxAge(pub u32);

impl header::Header for AccessControlMaxAge {
    #[inline]
    fn header_name() -> &'static str {
        "Access-Control-Max-Age"
    }

    fn parse_header(raw: &[Vec<u8>]) -> Option<AccessControlMaxAge> {
        header::parsing::from_one_raw_str(raw).map(AccessControlMaxAge)
    }
}

impl header::HeaderFormat for AccessControlMaxAge {
    fn fmt_header(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let AccessControlMaxAge(ref num) = *self;
        write!(f, "{}", num)
    }
}
