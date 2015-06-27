use std::fmt::{self, Display};
use std::str::FromStr;
use unicase::UniCase;

pub use self::ConnectionOption::{KeepAlive, Close, ConnectionHeader};

/// Values that can be in the `Connection` header.
#[derive(Clone, PartialEq, Debug)]
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
    ConnectionHeader(UniCase<String>),
}

impl FromStr for ConnectionOption {
    type Err = ();
    fn from_str(s: &str) -> Result<ConnectionOption, ()> {
        match s {
            "keep-alive" => Ok(KeepAlive),
            "close" => Ok(Close),
            s => Ok(ConnectionHeader(UniCase(s.to_owned())))
        }
    }
}

impl Display for ConnectionOption {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match *self {
            KeepAlive => "keep-alive",
            Close => "close",
            ConnectionHeader(UniCase(ref s)) => s.as_ref()
        })
    }
}

header! {
    #[doc="`Connection` header, defined in"]
    #[doc="[RFC7230](http://tools.ietf.org/html/rfc7230#section-6.1)"]
    #[doc=""]
    #[doc="The `Connection` header field allows the sender to indicate desired"]
    #[doc="control options for the current connection.  In order to avoid"]
    #[doc="confusing downstream recipients, a proxy or gateway MUST remove or"]
    #[doc="replace any received connection options before forwarding the"]
    #[doc="message."]
    #[doc=""]
    #[doc="# ABNF"]
    #[doc="```plain"]
    #[doc="Connection        = 1#connection-option"]
    #[doc="connection-option = token"]
    #[doc=""]
    #[doc="# Example values"]
    #[doc="* `close`"]
    #[doc="* `keep-alive`"]
    #[doc="* `upgrade`"]
    #[doc="```"]
    #[doc=""]
    #[doc="# Examples"]
    #[doc="```"]
    #[doc="use hyper::header::{Headers, Connection};"]
    #[doc=""]
    #[doc="let mut headers = Headers::new();"]
    #[doc="headers.set(Connection::keep_alive());"]
    #[doc="```"]
    #[doc="```"]
    #[doc="# extern crate hyper;"]
    #[doc="# extern crate unicase;"]
    #[doc="# fn main() {"]
    #[doc="// extern crate unicase;"]
    #[doc=""]
    #[doc="use hyper::header::{Headers, Connection, ConnectionOption};"]
    #[doc="use unicase::UniCase;"]
    #[doc=""]
    #[doc="let mut headers = Headers::new();"]
    #[doc="headers.set("]
    #[doc="    Connection(vec!["]
    #[doc="        ConnectionOption::ConnectionHeader(UniCase(\"upgrade\".to_owned())),"]
    #[doc="    ])"]
    #[doc=");"]
    #[doc="# }"]
    #[doc="```"]
    (Connection, "Connection") => (ConnectionOption)+

    test_connection {
        test_header!(test1, vec![b"close"]);
        test_header!(test2, vec![b"keep-alive"]);
        test_header!(test3, vec![b"upgrade"]);
    }
}

impl Connection {
    /// A constructor to easily create a `Connection: close` header.
    #[inline]
    pub fn close() -> Connection {
        Connection(vec![ConnectionOption::Close])
    }

    /// A constructor to easily create a `Connection: keep-alive` header.
    #[inline]
    pub fn keep_alive() -> Connection {
        Connection(vec![ConnectionOption::KeepAlive])
    }
}

bench_header!(close, Connection, { vec![b"close".to_vec()] });
bench_header!(keep_alive, Connection, { vec![b"keep-alive".to_vec()] });
bench_header!(header, Connection, { vec![b"authorization".to_vec()] });

#[cfg(test)]
mod tests {
    use super::{Connection,ConnectionHeader};
    use header::Header;
    use unicase::UniCase;

    fn parse_option(header: Vec<u8>) -> Connection {
        let val = vec![header];
        let connection: Connection = Header::parse_header(&val[..]).unwrap();
        connection
    }

    #[test]
    fn test_parse() {
        assert_eq!(Connection::close(),parse_option(b"close".to_vec()));
        assert_eq!(Connection::keep_alive(),parse_option(b"keep-alive".to_vec()));
        assert_eq!(Connection(vec![ConnectionHeader(UniCase("upgrade".to_owned()))]),
            parse_option(b"upgrade".to_vec()));
    }
}
