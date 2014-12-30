use std::fmt;

use header;
use header::shared;

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
/// # use hyper::header::common::Accept;
/// # use hyper::header::shared::QualityValue;
/// use hyper::mime::Mime;
/// use hyper::mime::TopLevel::Text;
/// use hyper::mime::SubLevel::{Html, Xml};
/// # let mut headers = Headers::new();
/// headers.set(Accept(vec![
///     QualityValue{value: Mime(Text, Html, vec![]), quality: 1f32},
///     QualityValue{value: Mime(Text, Xml, vec![]), quality: 1f32} ]));
/// ```
#[deriving(Clone, PartialEq, Show)]
pub struct Accept(pub Vec<shared::QualityValue<mime::Mime>>);

deref!(Accept -> Vec<shared::QualityValue<mime::Mime>>);

impl header::Header for Accept {
    fn header_name(_: Option<Accept>) -> &'static str {
        "Accept"
    }

    fn parse_header(raw: &[Vec<u8>]) -> Option<Accept> {
        // TODO: Return */* if no value is given.
        shared::from_comma_delimited(raw).map(Accept)
    }
}

impl header::HeaderFormat for Accept {
    fn fmt_header(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        shared::fmt_comma_delimited(fmt, self[])
    }
}

bench_header!(bench, Accept, { vec![b"text/plain; q=0.5, text/html".to_vec()] });
