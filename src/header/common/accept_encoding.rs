use std::default::Default;
use std::iter::IntoIterator;

use header::{Encoding, HttpContext, ToHeader, QualityItem};

/// The `Accept-Encoding` header
///
/// The `Accept-Encoding` header can be used by clients to indicate what
/// response encodings they accept.
#[derive(Clone, PartialEq, Debug)]
pub struct AcceptEncoding(pub Vec<QualityItem<Encoding>>);

impl_list_header!(AcceptEncoding,
                  "Accept-Encoding",
                  Vec<QualityItem<Encoding>>);

impl Default for AcceptEncoding {
    fn default() -> AcceptEncoding {
        AcceptEncoding(vec![])
    }
}

impl ToHeader for AcceptEncoding {
    fn from_iter<'a, I: IntoIterator<Item=&'a str>, C: HttpContext>(iterable: I, _: C) -> Option<AcceptEncoding> {
        let mut ret: AcceptEncoding = Default::default();
        for line in iterable.into_iter() {
            for value in line.split(',').map(|x| x.trim()).filter_map(|x| x.parse().ok()) {
                ret.0.push(value);
            }
        }
        Some(ret)
    }
}

#[cfg(test)]
mod tests {
    use std::default::Default;

    use test::Bencher;

    use header::{DummyHttpContext, Encoding, Header, ToHeader, qitem, Quality, QualityItem};

    use super::*;

    #[test]
    fn test_parse_header() {
        let a: AcceptEncoding = Header::parse_header([b"gzip;q=1.0, identity; q=0.5".to_vec()].as_slice()).unwrap();
        let b = AcceptEncoding(vec![
            qitem(Encoding::Gzip),
            QualityItem::new(Encoding::Identity, Quality(500)),
        ]);
        assert_eq!(a, b);
    }

    #[test]
    fn test_new_parsing() {
        let lines = vec!["gzip;q=1.0, identity; q=0.5"];
        let context: DummyHttpContext = Default::default();
        let header = AcceptEncoding::from_iter(lines, context).unwrap();
        let expected_header = AcceptEncoding(vec![
            qitem(Encoding::Gzip),
            QualityItem::new(Encoding::Identity, Quality(500)),
        ]);
        assert_eq!(header, expected_header);
    }

    #[bench]
    fn bench_parse_header(b: &mut Bencher) {
        b.iter(|| test_parse_header());
    }

    #[bench]
    fn bench_new_parsing(b: &mut Bencher) {
        b.iter(|| test_new_parsing());
    }
}
