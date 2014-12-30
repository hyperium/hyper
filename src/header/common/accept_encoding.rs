use std::fmt;

use header;
use header::shared::util;
use header::shared::encoding;
use header::shared::quality_value;

/// The `Accept-Encoding` header
///
/// The `Accept-Encoding` header can be used by clients to indicate what
/// response encodings they accept.
#[deriving(Clone, PartialEq, Show)]
pub struct AcceptEncoding(pub Vec<quality_value::QualityValue<encoding::Encoding>>);

deref!(AcceptEncoding -> Vec<quality_value::QualityValue<encoding::Encoding>>);

impl header::Header for AcceptEncoding {
    fn header_name(_: Option<AcceptEncoding>) -> &'static str {
        "AcceptEncoding"
    }

    fn parse_header(raw: &[Vec<u8>]) -> Option<AcceptEncoding> {
        util::from_comma_delimited(raw).map(AcceptEncoding)
    }
}

impl header::HeaderFormat for AcceptEncoding {
    fn fmt_header(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        util::fmt_comma_delimited(fmt, self[])
    }
}

#[test]
fn test_parse_header() {
    let a: AcceptEncoding = header::Header::parse_header([b"gzip;q=1.0, identity; q=0.5".to_vec()].as_slice()).unwrap();
    let b = AcceptEncoding(vec![
        quality_value::QualityValue{value: encoding::Gzip, quality: 1f32},
        quality_value::QualityValue{value: encoding::Identity, quality: 0.5f32},
    ]);
    assert_eq!(a, b);
}
