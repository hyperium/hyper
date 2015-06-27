use std::fmt::{self, Display};
use std::str::FromStr;
use unicase::UniCase;

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
    #[doc=""]
    #[doc="# Example values"]
    #[doc="* `HTTP/2.0, SHTTP/1.3, IRC/6.9, RTA/x11`"]
    #[doc=""]
    #[doc="# Examples"]
    #[doc="```"]
    #[doc="use hyper::header::{Headers, Upgrade, Protocol, ProtocolName};"]
    #[doc=""]
    #[doc="let mut headers = Headers::new();"]
    #[doc="headers.set(Upgrade(vec![Protocol::new(ProtocolName::WebSocket, None)]));"]
    #[doc="```"]
    #[doc="```"]
    #[doc="use hyper::header::{Headers, Upgrade, Protocol, ProtocolName};"]
    #[doc=""]
    #[doc="let mut headers = Headers::new();"]
    #[doc="headers.set("]
    #[doc="    Upgrade(vec!["]
    #[doc="        Protocol::new(ProtocolName::Http, Some(\"2.0\".to_owned())),"]
    #[doc="        Protocol::new(ProtocolName::Unregistered(\"SHTTP\".to_owned()),"]
    #[doc="            Some(\"1.3\".to_owned())),"]
    #[doc="        Protocol::new(ProtocolName::Unregistered(\"IRC\".to_owned()),"]
    #[doc="            Some(\"6.9\".to_owned())),"]
    #[doc="    ])"]
    #[doc=");"]
    #[doc="```"]
    (Upgrade, "Upgrade") => (Protocol)+

    test_upgrade {
        // Testcase from the RFC
        test_header!(
            test1,
            vec![b"HTTP/2.0, SHTTP/1.3, IRC/6.9, RTA/x11"],
            Some(Upgrade(vec![
                Protocol::new(ProtocolName::Http, Some("2.0".to_owned())),
                Protocol::new(ProtocolName::Unregistered("SHTTP".to_owned()),
                    Some("1.3".to_owned())),
                Protocol::new(ProtocolName::Unregistered("IRC".to_owned()), Some("6.9".to_owned())),
                Protocol::new(ProtocolName::Unregistered("RTA".to_owned()), Some("x11".to_owned())),
                ])));
        // Own tests
        test_header!(
            test2, vec![b"websocket"],
            Some(Upgrade(vec![Protocol::new(ProtocolName::WebSocket, None)])));
        #[test]
        fn test3() {
            let x: ::Result<Upgrade> = Header::parse_header(&[b"WEbSOCKet".to_vec()]);
            assert_eq!(x.ok(), Some(Upgrade(vec![Protocol::new(ProtocolName::WebSocket, None)])));
        }
    }
}

/// A protocol name used to identify a spefic protocol. Names are case-sensitive
/// except for the `WebSocket` value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProtocolName {
    /// `HTTP` value, Hypertext Transfer Protocol
    Http,
    /// `TLS` value, Transport Layer Security [RFC2817](http://tools.ietf.org/html/rfc2817)
    Tls,
    /// `WebSocket` value, matched case insensitively,Web Socket Protocol
    /// [RFC6455](http://tools.ietf.org/html/rfc6455)
    WebSocket,
    /// `h2c` value, HTTP/2 over cleartext TCP
    H2c,
    /// Any other protocol name not known to hyper
    Unregistered(String),
}

impl FromStr for ProtocolName {
    type Err = ();
    fn from_str(s: &str) -> Result<ProtocolName, ()> {
        Ok(match s {
            "HTTP" => ProtocolName::Http,
            "TLS" => ProtocolName::Tls,
            "h2c" => ProtocolName::H2c,
            _ => {
                if UniCase(s) == UniCase("websocket") {
                    ProtocolName::WebSocket
                } else {
                    ProtocolName::Unregistered(s.to_owned())
                }
            }
        })
    }
}

impl Display for ProtocolName {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match *self {
            ProtocolName::Http => "HTTP",
            ProtocolName::Tls => "TLS",
            ProtocolName::WebSocket => "websocket",
            ProtocolName::H2c => "h2c",
            ProtocolName::Unregistered(ref s) => s,
        })
    }
}

/// Protocols that appear in the `Upgrade` header field
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Protocol {
    /// The protocol identifier
    pub name: ProtocolName,
    /// The optional version of the protocol, often in the format "DIGIT.DIGIT" (e.g.. "1.2")
    pub version: Option<String>,
}

impl Protocol {
    /// Creates a new Protocol with the given name and version
    pub fn new(name: ProtocolName, version: Option<String>) -> Protocol {
        Protocol { name: name, version: version }
    }
}

impl FromStr for Protocol {
    type Err =();
    fn from_str(s: &str) -> Result<Protocol, ()> {
        let mut parts = s.splitn(2, '/');
        Ok(Protocol::new(try!(parts.next().unwrap().parse()), parts.next().map(|x| x.to_owned())))
    }
}

impl Display for Protocol {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        try!(fmt::Display::fmt(&self.name, f));
        if let Some(ref version) = self.version {
            try!(write!(f, "/{}", version));
        }
        Ok(())
    }
}

bench_header!(bench, Upgrade, { vec![b"HTTP/2.0, RTA/x11, websocket".to_vec()] });
