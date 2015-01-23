use header::{Header, HeaderFormat};
use std::fmt;
use header::Encoding;
use header::parsing::{from_comma_delimited, fmt_comma_delimited};

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
#[derive(Clone, PartialEq, Debug)]
pub struct TransferEncoding(pub Vec<Encoding>);

deref!(TransferEncoding => Vec<Encoding>);

impl Header for TransferEncoding {
    fn header_name() -> &'static str {
        "Transfer-Encoding"
    }

    fn parse_header(raw: &[Vec<u8>]) -> Option<TransferEncoding> {
        from_comma_delimited(raw).map(TransferEncoding)
    }
}

impl HeaderFormat for TransferEncoding {
    fn fmt_header(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt_comma_delimited(fmt, &self[])
    }
}

bench_header!(normal, TransferEncoding, { vec![b"chunked, gzip".to_vec()] });
bench_header!(ext, TransferEncoding, { vec![b"ext".to_vec()] });
