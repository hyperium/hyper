use header::Header;
use std::fmt::{mod, Show};
use super::from_one_raw_str;

/// The `User-Agent` header field.
///
/// They can contain any value, so it just wraps a `String`.
#[deriving(Clone, PartialEq, Show)]
pub struct UserAgent(pub String);

impl Header for UserAgent {
    fn header_name(_: Option<UserAgent>) -> &'static str {
        "User-Agent"
    }

    fn parse_header(raw: &[Vec<u8>]) -> Option<UserAgent> {
        from_one_raw_str(raw).map(|s| UserAgent(s))
    }

    fn fmt_header(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let UserAgent(ref value) = *self;
        value.fmt(fmt)
    }
}

