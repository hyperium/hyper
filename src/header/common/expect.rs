use std::fmt;
use std::str;

use unicase::UniCase;

use header::{Header, HeaderFormat};

/// The `Expect` header.
///
/// > The "Expect" header field in a request indicates a certain set of
/// > behaviors (expectations) that need to be supported by the server in
/// > order to properly handle this request.  The only such expectation
/// > defined by this specification is 100-continue.
/// >
/// >    Expect  = "100-continue"
///
/// # Example
/// ```
/// use hyper::header::{Headers, Expect};
/// let mut headers = Headers::new();
/// headers.set(Expect::Continue);
/// ```
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum Expect {
    /// The value `100-continue`.
    Continue
}

const EXPECT_CONTINUE: UniCase<&'static str> = UniCase("100-continue");

impl Header for Expect {
    fn header_name() -> &'static str {
        "Expect"
    }

    fn parse_header(raw: &[Vec<u8>]) -> ::Result<Expect> {
        if raw.len() == 1 {
            let text = unsafe {
                // safe because:
                // 1. we just checked raw.len == 1
                // 2. we don't actually care if it's utf8, we just want to
                //    compare the bytes with the "case" normalized. If it's not
                //    utf8, then the byte comparison will fail, and we'll return
                //    None. No big deal.
                str::from_utf8_unchecked(raw.get_unchecked(0))
            };
            if UniCase(text) == EXPECT_CONTINUE {
                Ok(Expect::Continue)
            } else {
                Err(::Error::Header)
            }
        } else {
            Err(::Error::Header)
        }
    }
}

impl HeaderFormat for Expect {
    fn fmt_header(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl fmt::Display for Expect {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("100-continue")
    }
}
