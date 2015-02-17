use mime::Mime;

use header::QualityItem;

/// The `Accept` header.
///
/// The `Accept` header is used to tell a server which content-types the client
/// is capable of using. It can be a comma-separated list of `Mime`s, and the
/// priority can be indicated with a `q` parameter.
///
/// Example:
///
/// ```
/// # use hyper::header::Headers;
/// # use hyper::header::Accept;
/// # use hyper::header::qitem;
/// use hyper::mime::Mime;
/// use hyper::mime::TopLevel::Text;
/// use hyper::mime::SubLevel::{Html, Xml};
/// # let mut headers = Headers::new();
/// headers.set(Accept(vec![
///     qitem(Mime(Text, Html, vec![])),
///     qitem(Mime(Text, Xml, vec![])) ]));
/// ```
#[derive(Clone, PartialEq, Debug)]
pub struct Accept(pub Vec<QualityItem<Mime>>);

impl_list_header!(Accept,
                  "Accept",
                  Vec<QualityItem<Mime>>);

#[cfg(test)]
mod tests {
    use mime::*;

    use header::{Header, QualityItem, qitem};
    use super::Accept;

    #[test]
    fn test_parse_header_no_quality() {
        let a: Accept = Header::parse_header([b"text/plain; charset=utf-8".to_vec()].as_slice()).unwrap();
        let b = Accept(vec![
            qitem(Mime(TopLevel::Text, SubLevel::Plain, vec![(Attr::Charset, Value::Utf8)])),
        ]);
        assert_eq!(a, b);
    }

    #[test]
    fn test_parse_header_with_quality() {
        let a: Accept = Header::parse_header([b"text/plain; charset=utf-8; q=0.5".to_vec()].as_slice()).unwrap();
        let b = Accept(vec![
            QualityItem::new(Mime(TopLevel::Text, SubLevel::Plain, vec![(Attr::Charset, Value::Utf8)]), 0.5f32),
        ]);
        assert_eq!(a, b);
    }
}

bench_header!(bench, Accept, { vec![b"text/plain; q=0.5, text/html".to_vec()] });
