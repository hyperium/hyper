use header::{Header, HeaderFormat};
use std::fmt::{self, Show};
use header::shared::util::from_one_raw_str;

/// The `Server` header field.
///
/// They can contain any value, so it just wraps a `String`.
#[derive(Clone, PartialEq, Show)]
pub struct Server(pub String);

deref!(Server -> String);

impl Header for Server {
    fn header_name(_: Option<Server>) -> &'static str {
        "Server"
    }

    fn parse_header(raw: &[Vec<u8>]) -> Option<Server> {
        from_one_raw_str(raw).map(|s| Server(s))
    }
}

impl HeaderFormat for Server {
    fn fmt_header(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let Server(ref value) = *self;
        value.fmt(fmt)
    }
}

bench_header!(bench, Server, { vec![b"Some String".to_vec()] });
