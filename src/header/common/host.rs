use header::Header;
use std::fmt::{mod, Show};
use super::from_one_raw_str;

/// The `Host` header.
///
/// HTTP/1.1 requires that all requests include a `Host` header, and so hyper
/// client requests add one automatically.
///
/// Currently is just a String, but it should probably become a better type,
/// like url::Host or something.
#[deriving(Clone, PartialEq, Show)]
pub struct Host(pub String);

impl Header for Host {
    fn header_name(_: Option<Host>) -> &'static str {
        "Host"
    }

    fn parse_header(raw: &[Vec<u8>]) -> Option<Host> {
        from_one_raw_str(raw).map(|s| Host(s))
    }

    fn fmt_header(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let Host(ref value) = *self;
        value.fmt(fmt)
    }
}

