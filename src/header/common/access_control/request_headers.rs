use std::fmt::{mod};

use header;
use header::shared;

#[deriving(Clone)]
struct AccessControlRequestHeaders(pub Vec<String>);

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
