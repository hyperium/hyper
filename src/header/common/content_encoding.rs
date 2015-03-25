use header::Encoding;

/// The `Content-Encoding` header.
///
/// This header describes the encoding of the message body. It can be
/// comma-separated, including multiple encodings.
///
/// ```notrust
/// Content-Encoding: gzip
/// ```
#[derive(Clone, PartialEq, Debug)]
pub struct ContentEncoding(pub Vec<Encoding>);

impl_list_header!(ContentEncoding,
                  "Content-Encoding",
                  Vec<Encoding>);

bench_header!(single, ContentEncoding, { vec![b"gzip".to_vec()] });
bench_header!(multiple, ContentEncoding, { vec![b"gzip, deflate".to_vec()] });
