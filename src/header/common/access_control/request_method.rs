use std::fmt::{mod};

use header;
use header::shared;
use method::Method;

#[deriving(Clone)]
struct AccessControlRequestMethod(pub Method);

impl header::Header for AccessControlRequestMethod {
    #[inline]
    fn header_name(_: Option<AccessControlRequestMethod>) -> &'static str {
        "Access-Control-Request-Method"
    }

    fn parse_header(raw: &[Vec<u8>]) -> Option<AccessControlRequestMethod> {
        shared::from_one_raw_str(raw).map(AccessControlRequestMethod)
    }
}

impl header::HeaderFormat for AccessControlRequestMethod {
    fn fmt_header(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let AccessControlRequestMethod(ref method) = *self;
        method.fmt(f)
    }
}
