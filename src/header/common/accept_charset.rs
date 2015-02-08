use header::{Charset, QualityItem};

/// The `Accept-Charset` header
///
/// The `Accept-Charset` header can be used by clients to indicate what
/// response charsets they accept.
#[derive(Clone, PartialEq, Debug)]
pub struct AcceptCharset(pub Vec<QualityItem<Charset>>);

impl_list_header!(AcceptCharset,
                  "Accept-Charset",
                  Vec<QualityItem<Charset>>);


#[test]
fn test_parse_header() {
    use header::{self, q};
    let a: AcceptCharset = header::Header::parse_header(
        [b"iso-8859-5, iso-8859-6;q=0.8".to_vec()].as_slice()).unwrap();
    let b = AcceptCharset(vec![
        QualityItem { item: Charset::Iso_8859_5, quality: q(1.0) },
        QualityItem { item: Charset::Iso_8859_6, quality: q(0.8) },
    ]);
    assert_eq!(format!("{}", a), format!("{}", b));
    assert_eq!(a, b);
}
