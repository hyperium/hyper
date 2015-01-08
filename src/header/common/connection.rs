use header::{Header, HeaderFormat};
use std::fmt;
use header::shared::util::{from_comma_delimited, fmt_comma_delimited};
use header::shared;

/// The `Connection` header.
#[derive(Clone, PartialEq, Show)]
pub struct Connection(pub Vec<shared::ConnectionOption>);

deref!(Connection -> Vec<shared::ConnectionOption>);

impl Header for Connection {
    fn header_name(_: Option<Connection>) -> &'static str {
        "Connection"
    }

    fn parse_header(raw: &[Vec<u8>]) -> Option<Connection> {
        from_comma_delimited(raw).map(|vec| Connection(vec))
    }
}

impl HeaderFormat for Connection {
    fn fmt_header(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let Connection(ref parts) = *self;
        fmt_comma_delimited(fmt, parts[])
    }
}

bench_header!(close, Connection, { vec![b"close".to_vec()] });
bench_header!(keep_alive, Connection, { vec![b"keep-alive".to_vec()] });
bench_header!(header, Connection, { vec![b"authorization".to_vec()] });
