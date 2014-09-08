use header::Header;
use std::fmt::{mod, Show};
use super::from_one_raw_str;
use std::from_str::FromStr;

/// The `Connection` header.
///
/// Describes whether the socket connection should be closed or reused after
/// this request/response is completed.
#[deriving(Clone, PartialEq, Show)]
pub enum Connection {
    /// The `keep-alive` connection value.
    KeepAlive,
    /// The `close` connection value.
    Close
}

impl FromStr for Connection {
    fn from_str(s: &str) -> Option<Connection> {
        debug!("Connection::from_str =? {}", s);
        match s {
            "keep-alive" => Some(KeepAlive),
            "close" => Some(Close),
            _ => None
        }
    }
}

impl Header for Connection {
    fn header_name(_: Option<Connection>) -> &'static str {
        "connection"
    }

    fn parse_header(raw: &[Vec<u8>]) -> Option<Connection> {
        from_one_raw_str(raw)
    }

    fn fmt_header(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            KeepAlive => "keep-alive",
            Close => "close",
        }.fmt(fmt)
    }
}

