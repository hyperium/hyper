use header::HttpDate;

/// The `If-Unmodified-Since` header field.
#[derive(Copy, PartialEq, Clone, Debug)]
pub struct IfUnmodifiedSince(pub HttpDate);

impl_header!(IfUnmodifiedSince, "If-Unmodified-Since", HttpDate);

bench_header!(imf_fixdate, IfUnmodifiedSince, { vec![b"Sun, 07 Nov 1994 08:48:37 GMT".to_vec()] });
bench_header!(rfc_850, IfUnmodifiedSince, { vec![b"Sunday, 06-Nov-94 08:49:37 GMT".to_vec()] });
bench_header!(asctime, IfUnmodifiedSince, { vec![b"Sun Nov  6 08:49:37 1994".to_vec()] });
