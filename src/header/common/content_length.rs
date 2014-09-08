use header::Header;
use std::fmt::{mod, Show};
use super::from_one_raw_str;

/// The `Content-Length` header.
///
/// Simply a wrapper around a `uint`.
#[deriving(Clone, PartialEq, Show)]
pub struct ContentLength(pub uint);

impl Header for ContentLength {
    fn header_name(_: Option<ContentLength>) -> &'static str {
        "content-length"
    }

    fn parse_header(raw: &[Vec<u8>]) -> Option<ContentLength> {
        from_one_raw_str(raw).map(|u| ContentLength(u))
    }

    fn fmt_header(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let ContentLength(ref value) = *self;
        value.fmt(fmt)
    }
}

