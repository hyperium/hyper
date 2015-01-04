use std::fmt::{mod};

use header;
use header::shared;

#[deriving(Clone)]
struct AccessControlAllowHeaders(pub Vec<String>);

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
