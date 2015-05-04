use header::{Header, HeaderFormat};
use std::fmt::{self, Display};
use std::str::FromStr;
use header::parsing::{from_comma_delimited, fmt_comma_delimited};
use unicase::UniCase;

pub use self::ConnectionOption::{KeepAlive, Close, ConnectionHeader};

/// `Connection` header, defined in [RFC7230](https://tools.ietf.org/html/rfc7230#section-6.1)
///
/// The `Connection` header field allows the sender to indicate desired
/// control options for the current connection.  In order to avoid
/// confusing downstream recipients, a proxy or gateway MUST remove or
/// replace any received connection options before forwarding the
/// message.
///
/// # ABNF
/// ```plain
/// Connection        = 1#connection-option
/// connection-option = token
/// ```
///
/// # Example values
/// * `close`
/// * `upgrade`
/// * `keep-alive`
#[derive(Clone, PartialEq, Debug)]
pub struct Connection(pub Vec<ConnectionOption>);

deref!(Connection => Vec<ConnectionOption>);

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
        Ok(match s {
            "keep-alive" => KeepAlive,
            "close" => Close,
            s => ConnectionHeader(UniCase(s.to_string())),
        })
    }
}

impl Display for ConnectionOption {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match *self {
            KeepAlive => "keep-alive",
            Close => "close",
            ConnectionHeader(UniCase(ref s)) => s,
        })
    }
}

impl Header for Connection {
    fn header_name() -> &'static str {
        "Connection"
    }

    fn parse_header(raw: &[Vec<u8>]) -> Option<Connection> {
        from_comma_delimited(raw).map(|vec| Connection(vec))
    }
}

impl HeaderFormat for Connection {
    fn fmt_header(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let Connection(ref parts) = *self;
        fmt_comma_delimited(f, &parts[..])
    }
}

bench_header!(close, Connection, { vec![b"close".to_vec()] });
bench_header!(keep_alive, Connection, { vec![b"keep-alive".to_vec()] });
bench_header!(header, Connection, { vec![b"authorization".to_vec()] });
