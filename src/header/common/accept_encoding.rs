use header::{self, Encoding, QualityItem};

/// The `Accept-Encoding` header
///
/// The `Accept-Encoding` header can be used by clients to indicate what
/// response encodings they accept.
#[derive(Clone, PartialEq, Debug)]
pub struct AcceptEncoding(pub Vec<QualityItem<Encoding>>);

impl_list_header!(AcceptEncoding,
                  "Accept-Encoding",
                  Vec<QualityItem<Encoding>>);

#[test]
fn test_parse_header() {
    let a: AcceptEncoding = header::Header::parse_header([b"gzip;q=1.0, identity; q=0.5".to_vec()].as_slice()).unwrap();
    let b = AcceptEncoding(vec![
        QualityItem{item: Encoding::Gzip, quality: 1f32},
        QualityItem{item: Encoding::Identity, quality: 0.5f32},
    ]);
    assert_eq!(a, b);
}
