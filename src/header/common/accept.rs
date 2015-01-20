use std::fmt;

use header;
use header::parsing;

use mime;

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
#[derive(Clone, PartialEq, Show)]
pub struct Accept(pub Vec<header::QualityItem<mime::Mime>>);

deref!(Accept => Vec<header::QualityItem<mime::Mime>>);

impl header::Header for Accept {
    fn header_name(_: Option<Accept>) -> &'static str {
        "Accept"
    }

    fn parse_header(raw: &[Vec<u8>]) -> Option<Accept> {
        // TODO: Return */* if no value is given.
        parsing::from_comma_delimited(raw).map(Accept)
    }
}

impl header::HeaderFormat for Accept {
    fn fmt_header(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        parsing::fmt_comma_delimited(fmt, &self[])
    }
}

bench_header!(bench, Accept, { vec![b"text/plain; q=0.5, text/html".to_vec()] });

#[test]
fn test_parse_header_no_quality() {
    let a: Accept = header::Header::parse_header([b"text/plain; charset=utf-8".to_vec()].as_slice()).unwrap();
    let b = Accept(vec![
        header::QualityItem{item: mime::Mime(mime::TopLevel::Text, mime::SubLevel::Plain, vec![(mime::Attr::Charset, mime::Value::Utf8)]), quality: 1f32},
    ]);
    assert_eq!(a, b);
}

#[test]
fn test_parse_header_with_quality() {
    let a: Accept = header::Header::parse_header([b"text/plain; charset=utf-8; q=0.5".to_vec()].as_slice()).unwrap();
    let b = Accept(vec![
        header::QualityItem{item: mime::Mime(mime::TopLevel::Text, mime::SubLevel::Plain, vec![(mime::Attr::Charset, mime::Value::Utf8)]), quality: 0.5f32},
    ]);
    assert_eq!(a, b);
}
