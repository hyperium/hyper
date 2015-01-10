use std::fmt::{self, Show};

use header::{Header, HeaderFormat};
use header::shared::util::from_one_raw_str;

/// The `Content-Length` header.
///
/// Simply a wrapper around a `usize`.
#[derive(Copy, Clone, PartialEq, Show)]
pub struct ContentLength(pub usize);

deref!(ContentLength => usize);

impl Header for ContentLength {
    fn header_name(_: Option<ContentLength>) -> &'static str {
        "Content-Length"
    }

    fn parse_header(raw: &[Vec<u8>]) -> Option<ContentLength> {
        from_one_raw_str(raw).map(|u| ContentLength(u))
    }
}

impl HeaderFormat for ContentLength {
    fn fmt_header(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let ContentLength(ref value) = *self;
        value.fmt(fmt)
    }
}

impl ContentLength {
    /// Returns the wrapped length.
    #[deprecated = "use Deref instead"]
    #[inline]
    pub fn len(&self) -> usize {
        **self
    }
}

bench_header!(bench, ContentLength, { vec![b"42349984".to_vec()] });

