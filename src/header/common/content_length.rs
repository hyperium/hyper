use std::fmt;

use header::{Header, Raw, parsing};

/// `Content-Length` header, defined in
/// [RFC7230](http://tools.ietf.org/html/rfc7230#section-3.3.2)
///
/// When a message does not have a `Transfer-Encoding` header field, a
/// Content-Length header field can provide the anticipated size, as a
/// decimal number of octets, for a potential payload body.  For messages
/// that do include a payload body, the Content-Length field-value
/// provides the framing information necessary for determining where the
/// body (and message) ends.  For messages that do not include a payload
/// body, the Content-Length indicates the size of the selected
/// representation.
///
/// Note that setting this header will *remove* any previously set
/// `Transfer-Encoding` header, in accordance with
/// [RFC7230](http://tools.ietf.org/html/rfc7230#section-3.3.2):
///
/// > A sender MUST NOT send a Content-Length header field in any message
/// that > contains a Transfer-Encoding header field.
///
/// # ABNF
/// ```plain
/// Content-Length = 1*DIGIT
/// ```
///
/// # Example values
/// * `3495`
///
/// # Example
/// ```
/// use hyper::header::{Headers, ContentLength};
///
/// let mut headers = Headers::new();
/// headers.set(ContentLength(1024u64));
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContentLength(pub u64);

impl Header for ContentLength {
    #[inline]
    fn header_name() -> &'static str {
        static NAME: &'static str = "Content-Length";
        NAME
    }

    fn parse_header(raw: &Raw) -> ::Result<ContentLength> {
        // If multiple Content-Length headers were sent, everything can still
        // be alright if they all contain the same value, and all parse
        // correctly. If not, then it's an error.
        raw.iter()
            .map(parsing::from_raw_str)
            .fold(None, |prev, x| {
                match (prev, x) {
                    (None, x) => Some(x),
                    (e @ Some(Err(_)), _ ) => e,
                    (Some(Ok(prev)), Ok(x)) if prev == x => Some(Ok(prev)),
                    _ => Some(Err(::Error::Header)),
                }
            })
            .unwrap_or(Err(::Error::Header))
            .map(ContentLength)
    }

    #[inline]
    fn fmt_header(&self, f: &mut ::header::Formatter) -> fmt::Result {
        f.fmt_line(self)
    }
}

impl fmt::Display for ContentLength {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

__hyper__deref!(ContentLength => u64);

__hyper__tm!(ContentLength, tests {
    // Testcase from RFC
    test_header!(test1, vec![b"3495"], Some(HeaderField(3495)));

    test_header!(test_invalid, vec![b"34v95"], None);

    // Can't use the test_header macro because "5, 5" gets cleaned to "5".
    #[test]
    fn test_duplicates() {
        let parsed = HeaderField::parse_header(&vec![b"5".to_vec(),
                                                 b"5".to_vec()].into()).unwrap();
        assert_eq!(parsed, HeaderField(5));
        assert_eq!(format!("{}", parsed), "5");
    }

    test_header!(test_duplicates_vary, vec![b"5", b"6", b"5"], None);
});

bench_header!(bench, ContentLength, { vec![b"42349984".to_vec()] });
