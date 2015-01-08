use header::{Header, HeaderFormat};
use std::fmt;
use header::shared::util::{from_comma_delimited, fmt_comma_delimited};
use header::shared;

/// The `Transfer-Encoding` header.
///
/// This header describes the encoding of the message body. It can be
/// comma-separated, including multiple encodings.
///
/// ```notrust
/// Transfer-Encoding: gzip, chunked
/// ```
///
/// According to the spec, if a `Content-Length` header is not included,
/// this header should include `chunked` as the last encoding.
///
/// The implementation uses a vector of `Encoding` values.
#[derive(Clone, PartialEq, Show)]
pub struct TransferEncoding(pub Vec<shared::Encoding>);

deref!(TransferEncoding -> Vec<shared::Encoding>);

impl Header for TransferEncoding {
    fn header_name(_: Option<TransferEncoding>) -> &'static str {
        "Transfer-Encoding"
    }

    fn parse_header(raw: &[Vec<u8>]) -> Option<TransferEncoding> {
        from_comma_delimited(raw).map(TransferEncoding)
    }
}

impl HeaderFormat for TransferEncoding {
    fn fmt_header(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt_comma_delimited(fmt, self[])
    }
}

bench_header!(normal, TransferEncoding, { vec![b"chunked, gzip".to_vec()] });
bench_header!(ext, TransferEncoding, { vec![b"ext".to_vec()] });
