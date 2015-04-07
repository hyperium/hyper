use std::fmt;
use std::str::FromStr;
use unicase::UniCase;

use self::Protocol::{WebSocket, ProtocolExt};

header! {
    #[doc="`Upgrade` header, defined in [RFC7230](http://tools.ietf.org/html/rfc7230#section-6.7)"]
    #[doc=""]
    #[doc="The `Upgrade` header field is intended to provide a simple mechanism"]
    #[doc="for transitioning from HTTP/1.1 to some other protocol on the same"]
    #[doc="connection.  A client MAY send a list of protocols in the Upgrade"]
    #[doc="header field of a request to invite the server to switch to one or"]
    #[doc="more of those protocols, in order of descending preference, before"]
    #[doc="sending the final response.  A server MAY ignore a received Upgrade"]
    #[doc="header field if it wishes to continue using the current protocol on"]
    #[doc="that connection.  Upgrade cannot be used to insist on a protocol"]
    #[doc="change."]
    #[doc=""]
    #[doc="# ABNF"]
    #[doc="```plain"]
    #[doc="Upgrade          = 1#protocol"]
    #[doc=""]
    #[doc="protocol         = protocol-name [\"/\" protocol-version]"]
    #[doc="protocol-name    = token"]
    #[doc="protocol-version = token"]
    #[doc="```"]
    (Upgrade, "Upgrade") => (Protocol)+
}

/// Protocol values that can appear in the Upgrade header.
#[derive(Clone, PartialEq, Debug)]
pub enum Protocol {
    /// The websocket protocol.
    WebSocket,
    /// Some other less common protocol.
    ProtocolExt(String),
}

impl FromStr for Protocol {
    type Err = ();
    fn from_str(s: &str) -> Result<Protocol, ()> {
        if UniCase(s) == UniCase("websocket") {
            Ok(WebSocket)
        }
        else {
            Ok(ProtocolExt(s.to_string()))
        }
    }
}

impl fmt::Display for Protocol {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "{}", match *self {
            WebSocket => "websocket",
            ProtocolExt(ref s) => s.as_ref()
        })
    }
}

bench_header!(bench, Upgrade, { vec![b"HTTP/2.0, RTA/x11, websocket".to_vec()] });
