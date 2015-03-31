use header::HttpDate;

/// The `LastModified` header field.
#[derive(Copy, PartialEq, Clone, Debug)]
pub struct LastModified(pub HttpDate);

impl_header!(LastModified, "Last-Modified", HttpDate);

bench_header!(imf_fixdate, LastModified, { vec![b"Sun, 07 Nov 1994 08:48:37 GMT".to_vec()] });
bench_header!(rfc_850, LastModified, { vec![b"Sunday, 06-Nov-94 08:49:37 GMT".to_vec()] });
bench_header!(asctime, LastModified, { vec![b"Sun Nov  6 08:49:37 1994".to_vec()] });
