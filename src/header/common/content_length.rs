use std::fmt;

use header::{HeaderFormat, Header, parsing};

#[doc="`Content-Length` header, defined in"]
#[doc="[RFC7230](http://tools.ietf.org/html/rfc7230#section-3.3.2)"]
#[doc=""]
#[doc="When a message does not have a `Transfer-Encoding` header field, a"]
#[doc="Content-Length header field can provide the anticipated size, as a"]
#[doc="decimal number of octets, for a potential payload body.  For messages"]
#[doc="that do include a payload body, the Content-Length field-value"]
#[doc="provides the framing information necessary for determining where the"]
#[doc="body (and message) ends.  For messages that do not include a payload"]
#[doc="body, the Content-Length indicates the size of the selected"]
#[doc="representation."]
#[doc=""]
#[doc="# ABNF"]
#[doc="```plain"]
#[doc="Content-Length = 1*DIGIT"]
#[doc="```"]
#[doc=""]
#[doc="# Example values"]
#[doc="* `3495`"]
#[doc=""]
#[doc="# Example"]
#[doc="```"]
#[doc="use hyper::header::{Headers, ContentLength};"]
#[doc=""]
#[doc="let mut headers = Headers::new();"]
#[doc="headers.set(ContentLength(1024u64));"]
#[doc="```"]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContentLength(pub u64);

impl Header for ContentLength {
    #[inline]
    fn header_name() -> &'static str {
        "Content-Length"
    }
    fn parse_header(raw: &[Vec<u8>]) -> ::Result<ContentLength> {
        // If multiple Content-Length headers were sent, everything can still
        // be alright if they all contain the same value, and all parse
        // correctly. If not, then it's an error.
        raw.iter()
            .map(::std::ops::Deref::deref)
            .map(parsing::from_raw_str)
            .fold(None, |prev, x| {
                match (prev, x) {
                    (None, x) => Some(x),
                    (e@Some(Err(_)), _ ) => e,
                    (Some(Ok(prev)), Ok(x)) if prev == x => Some(Ok(prev)),
                    _ => Some(Err(::Error::Header))
                }
            })
            .unwrap_or(Err(::Error::Header))
            .map(ContentLength)
    }
}

impl HeaderFormat for ContentLength {
    #[inline]
    fn fmt_header(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl fmt::Display for ContentLength {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

__hyper__deref!(ContentLength => u64);
__hyper_generate_header_serialization!(ContentLength);

__hyper__tm!(ContentLength, tests {
    // Testcase from RFC
    test_header!(test1, vec![b"3495"], Some(HeaderField(3495)));

    test_header!(test_invalid, vec![b"34v95"], None);
    test_header!(test_duplicates, vec![b"5", b"5"], Some(HeaderField(5)));
    test_header!(test_duplicates_vary, vec![b"5", b"6", b"5"], None);
});

bench_header!(bench, ContentLength, { vec![b"42349984".to_vec()] });
