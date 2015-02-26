use header::Encoding;

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

impl_list_header!(TransferEncoding,
                  "Transfer-Encoding",
                  Vec<Encoding>);

bench_header!(normal, TransferEncoding, { vec![b"chunked, gzip".to_vec()] });
bench_header!(ext, TransferEncoding, { vec![b"ext".to_vec()] });
