use std::fmt;
use std::str;

/// Values that can be in the `Connection` header.
#[derive(Clone, PartialEq)]
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

impl str::FromStr for ConnectionOption {
    fn from_str(s: &str) -> Option<ConnectionOption> {
        match s {
            "keep-alive" => Some(ConnectionOption::KeepAlive),
            "close" => Some(ConnectionOption::Close),
            s => Some(ConnectionOption::ConnectionHeader(s.to_string()))
        }
    }
}

impl fmt::Show for ConnectionOption {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ConnectionOption::KeepAlive => "keep-alive",
            ConnectionOption::Close => "close",
            ConnectionOption::ConnectionHeader(ref s) => s.as_slice()
        }.fmt(fmt)
    }
}
