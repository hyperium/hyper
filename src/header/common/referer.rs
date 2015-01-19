use header::{Header, HeaderFormat};
use std::fmt::{self, Show};
use header::shared::util::from_one_raw_str;

/// The `Referer` header.
///
/// The Referer header is used by user agents to inform server about
/// the page URL user has came from.
///
/// See alse [RFC 1945, section 10.13](http://tools.ietf.org/html/rfc1945#section-10.13).
///
/// Currently just a string, but maybe better replace it with url::Url or something like it.
#[derive(Clone, PartialEq, Show)]
pub struct Referer(pub String);

deref!(Referer => String);

impl Header for Referer {
    fn header_name(_: Option<Referer>) -> &'static str {
        "Referer"
    }

    fn parse_header(raw: &[Vec<u8>]) -> Option<Referer> {
        from_one_raw_str(raw).map(|s| Referer(s))
    }
}

impl HeaderFormat for Referer {
    fn fmt_header(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let Referer(ref value) = *self;
        value.fmt(fmt)
    }
}

bench_header!(bench, Referer, { vec![b"http://foo.com/hello:3000".to_vec()] });
