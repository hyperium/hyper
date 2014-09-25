use header::Header;
use std::fmt::{mod, Show};
use super::{from_comma_delimited, fmt_comma_delimited};
use std::from_str::FromStr;

/// The `Upgrade` header.
#[deriving(Clone, PartialEq, Show)]
pub struct Upgrade(Vec<Protocol>);

/// Protocol values that can appear in the Upgrade header.
#[deriving(Clone, PartialEq)]
pub enum Protocol {
    /// The websocket protocol.
    WebSocket,
    /// Some other less common protocol.
    ProtocolExt(String),
}

impl FromStr for Protocol {
    fn from_str(s: &str) -> Option<Protocol> {
        match s {
            "websocket" => Some(WebSocket),
            s => Some(ProtocolExt(s.to_string()))
        }
    }
}

impl fmt::Show for Protocol {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            WebSocket => "websocket",
            ProtocolExt(ref s) => s.as_slice()
        }.fmt(fmt)
    }
}

impl Header for Upgrade {
    fn header_name(_: Option<Upgrade>) -> &'static str {
        "Upgrade"
    }

    fn parse_header(raw: &[Vec<u8>]) -> Option<Upgrade> {
        from_comma_delimited(raw).map(|vec| Upgrade(vec))
    }

    fn fmt_header(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let Upgrade(ref parts) = *self;
        fmt_comma_delimited(fmt, parts[])
    }
}

