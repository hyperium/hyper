use std::fmt;

use header::{Header, HeaderFormat};

/// The `Expect` header.
///
/// > The "Expect" header field in a request indicates a certain set of
/// > behaviors (expectations) that need to be supported by the server in
/// > order to properly handle this request.  The only such expectation
/// > defined by this specification is 100-continue.
/// >
/// >    Expect  = "100-continue"
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum Expect {
    /// The value `100-continue`.
    Continue
}

impl Header for Expect {
    fn header_name() -> &'static str {
        "Expect"
    }

    fn parse_header(raw: &[Vec<u8>]) -> Option<Expect> {
        if &[b"100-continue"] == raw {
            Some(Expect::Continue)
        } else {
            None
        }
    }
}

impl HeaderFormat for Expect {
    fn fmt_header(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("100-continue")
    }
}
