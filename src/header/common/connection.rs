use header::Header;
use std::fmt::{mod, Show};
use super::{from_comma_delimited, fmt_comma_delimited};
use std::from_str::FromStr;

/// The `Connection` header.
#[deriving(Clone, PartialEq, Show)]
pub struct Connection(Vec<ConnectionOption>);

/// Values that can be in the `Connection` header.
#[deriving(Clone, PartialEq)]
pub enum ConnectionOption {
    /// The `keep-alive` connection value.
    KeepAlive,
    /// The `close` connection value.
    Close,
    /// Values in the Connection header that are supposed to be names of other Headers.
    ///
    /// > When a header field aside from Connection is used to supply control
    /// > information for or about the current connection, the sender MUST list
    /// > the corresponding field-name within the Connection header field.
    // TODO: it would be nice if these "Strings" could be stronger types, since
    // they are supposed to relate to other Header fields (which we have strong
    // types for).
    ConnectionHeader(String),
}

impl FromStr for ConnectionOption {
    fn from_str(s: &str) -> Option<ConnectionOption> {
        match s {
            "keep-alive" => Some(KeepAlive),
            "close" => Some(Close),
            s => Some(ConnectionHeader(s.to_string()))
        }
    }
}

impl fmt::Show for ConnectionOption {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            KeepAlive => "keep-alive",
            Close => "close",
            ConnectionHeader(ref s) => s.as_slice()
        }.fmt(fmt)
    }
}

impl Header for Connection {
    fn header_name(_: Option<Connection>) -> &'static str {
        "Connection"
    }

    fn parse_header(raw: &[Vec<u8>]) -> Option<Connection> {
        from_comma_delimited(raw).map(|vec| Connection(vec))
    }

    fn fmt_header(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let Connection(ref parts) = *self;
        fmt_comma_delimited(fmt, parts[])
    }
}

