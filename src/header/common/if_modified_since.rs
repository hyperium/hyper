use header::HttpDate;

/// The `If-Modified-Since` header field.
#[derive(Copy, PartialEq, Clone, Debug)]
pub struct IfModifiedSince(pub HttpDate);
impl_header!(IfModifiedSince, "If-Modified-Since", HttpDate);

bench_header!(imf_fixdate, IfModifiedSince, { vec![b"Sun, 07 Nov 1994 08:48:37 GMT".to_vec()] });
bench_header!(rfc_850, IfModifiedSince, { vec![b"Sunday, 06-Nov-94 08:49:37 GMT".to_vec()] });
bench_header!(asctime, IfModifiedSince, { vec![b"Sun Nov  6 08:49:37 1994".to_vec()] });
