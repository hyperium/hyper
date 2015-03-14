use header::{Encoding, QualityItem};

/// The `Accept-Encoding` header
///
/// The `Accept-Encoding` header can be used by clients to indicate what
/// response encodings they accept.
#[derive(Clone, PartialEq, Debug)]
pub struct AcceptEncoding(pub Vec<QualityItem<Encoding>>);

impl_list_header!(AcceptEncoding,
                  "Accept-Encoding",
                  Vec<QualityItem<Encoding>>);

#[cfg(test)]
mod tests {
    use header::{Encoding, Header, qitem, Quality, QualityItem};

    use super::*;

    #[test]
    fn test_parse_header() {
        let a: AcceptEncoding = Header::parse_header([b"gzip;q=1.0, identity; q=0.5".to_vec()].as_ref()).unwrap();
        let b = AcceptEncoding(vec![
            qitem(Encoding::Gzip),
            QualityItem::new(Encoding::Identity, Quality(500)),
        ]);
        assert_eq!(a, b);
    }
}
