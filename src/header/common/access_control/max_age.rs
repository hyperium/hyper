use std::fmt;

use header;
use header::shared;

#[derive(Clone)]
struct AccessControlMaxAge(pub u32);

impl header::Header for AccessControlMaxAge {
    #[inline]
    fn header_name(_: Option<AccessControlMaxAge>) -> &'static str {
        "Access-Control-Max-Age"
    }

    fn parse_header(raw: &[Vec<u8>]) -> Option<AccessControlMaxAge> {
        shared::from_one_raw_str(raw).map(AccessControlMaxAge)
    }
}

impl header::HeaderFormat for AccessControlMaxAge {
    fn fmt_header(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let AccessControlMaxAge(ref num) = *self;
        write!(f, "{}", num)
    }
}
