use std::fmt;

use header::{Header, HeaderFormat};
use header::parsing::from_one_raw_str;

/// The `Content-Length` header.
///
/// Simply a wrapper around a `usize`.
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct ContentLength(pub u64);

deref!(ContentLength => u64);

impl Header for ContentLength {
    fn header_name() -> &'static str {
        "Content-Length"
    }

    fn parse_header(raw: &[Vec<u8>]) -> Option<ContentLength> {
        from_one_raw_str(raw).map(|u| ContentLength(u))
    }
}

impl HeaderFormat for ContentLength {
    fn fmt_header(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
       fmt::Display::fmt(&self.0, fmt)
    }
}

bench_header!(bench, ContentLength, { vec![b"42349984".to_vec()] });

