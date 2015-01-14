use header::{self, shared};

/// The `Accept-Encoding` header
///
/// The `Accept-Encoding` header can be used by clients to indicate what
/// response encodings they accept.
#[derive(Clone, PartialEq, Show)]
pub struct AcceptEncoding(pub Vec<shared::QualityItem<shared::Encoding>>);

impl_list_header!(AcceptEncoding,
                  "Accept-Encoding",
                  Vec<shared::QualityItem<shared::Encoding>>);

#[test]
fn test_parse_header() {
    let a: AcceptEncoding = header::Header::parse_header([b"gzip;q=1.0, identity; q=0.5".to_vec()].as_slice()).unwrap();
    let b = AcceptEncoding(vec![
        shared::QualityItem{item: shared::Gzip, quality: 1f32},
        shared::QualityItem{item: shared::Identity, quality: 0.5f32},
    ]);
    assert_eq!(a, b);
}
