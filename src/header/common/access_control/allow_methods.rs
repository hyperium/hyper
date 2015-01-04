use std::fmt::{mod};

use header;
use header::shared;

#[deriving(Clone)]
struct AccessControlAllowMethods(pub Vec<Method>);

impl header::Header for AccessControlAllowMethods {
    #[inline]
    fn header_name(_: Option<AccessControlAllowMethods>) -> &'static str {
        "Access-Control-Allow-Methods"
    }

    fn parse_header(raw: &[Vec<u8>]) -> Option<AccessControlAllowMethods> {
        shared::from_comma_delimited(raw).map(AccessControlAllowMethods)
    }
}

impl header::HeaderFormat for AccessControlAllowMethods {
    fn fmt_header(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let AccessControlAllowMethods(ref parts) = *self;
        shared::fmt_comma_delimited(f, parts.as_slice())
    }
}
