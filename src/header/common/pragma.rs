use std::fmt;
use std::ascii::AsciiExt;

use header::{Header, HeaderFormat, parsing};

/// The `Pragma` header defined by HTTP/1.0.
///
/// > The "Pragma" header field allows backwards compatibility with
/// > HTTP/1.0 caches, so that clients can specify a "no-cache" request
/// > that they will understand (as Cache-Control was not defined until
/// > HTTP/1.1).  When the Cache-Control header field is also present and
/// > understood in a request, Pragma is ignored.

/// > In HTTP/1.0, Pragma was defined as an extensible field for
/// > implementation-specified directives for recipients.  This
/// > specification deprecates such extensions to improve interoperability.
///
/// Spec: https://tools.ietf.org/html/rfc7234#section-5.4
#[derive(Clone, PartialEq, Debug)]
pub enum Pragma {
    /// Corresponds to the `no-cache` value.
    NoCache,
    /// Every value other than `no-cache`.
    Ext(String),
}

impl Header for Pragma {
    fn header_name() -> &'static str {
        "Pragma"
    }

    fn parse_header(raw: &[Vec<u8>]) -> Option<Pragma> {
        parsing::from_one_raw_str(raw).and_then(|s: String| {
            let slice = &s.to_ascii_lowercase()[..];
            match slice {
                "" => None,
                "no-cache" => Some(Pragma::NoCache),
                _ => Some(Pragma::Ext(s)),
            }
        })
    }
}

impl HeaderFormat for Pragma {
    fn fmt_header(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Pragma::NoCache => write!(f, "no-cache"),
            Pragma::Ext(ref string) => write!(f, "{}", string),
        }
    }
}

#[test]
fn test_parse_header() {
    let a: Pragma = Header::parse_header([b"no-cache".to_vec()].as_slice()).unwrap();
    let b = Pragma::NoCache;
    assert_eq!(a, b);
    let c: Pragma = Header::parse_header([b"FoObar".to_vec()].as_slice()).unwrap();
    let d = Pragma::Ext("FoObar".to_string());
    assert_eq!(c, d);
}
