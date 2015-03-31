use header::HttpDate;

/// The `Date` header field.
#[derive(Copy, PartialEq, Clone, Debug)]
pub struct Date(pub HttpDate);

impl_header!(Date, "Date", HttpDate);

bench_header!(imf_fixdate, Date, { vec![b"Sun, 07 Nov 1994 08:48:37 GMT".to_vec()] });
bench_header!(rfc_850, Date, { vec![b"Sunday, 06-Nov-94 08:49:37 GMT".to_vec()] });
bench_header!(asctime, Date, { vec![b"Sun Nov  6 08:49:37 1994".to_vec()] });
