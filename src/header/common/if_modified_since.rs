use std::fmt::{self, Show};
use std::str::FromStr;
use time::Tm;
use header::{Header, HeaderFormat};
use header::shared::util::from_one_raw_str;
use header::shared::time::tm_from_str;

/// The `If-Modified-Since` header field.
#[derive(Copy, PartialEq, Clone)]
pub struct IfModifiedSince(pub Tm);

deref!(IfModifiedSince -> Tm);

impl Header for IfModifiedSince {
    fn header_name(_: Option<IfModifiedSince>) -> &'static str {
        "If-Modified-Since"
    }

    fn parse_header(raw: &[Vec<u8>]) -> Option<IfModifiedSince> {
        from_one_raw_str(raw)
    }
}


impl HeaderFormat for IfModifiedSince {
    fn fmt_header(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let tm = **self;
        match tm.tm_utcoff {
            0 => tm.rfc822().fmt(fmt),
            _ => tm.to_utc().rfc822().fmt(fmt)
        }
    }
}

impl FromStr for IfModifiedSince {
    fn from_str(s: &str) -> Option<IfModifiedSince> {
        tm_from_str(s).map(IfModifiedSince)
    }
}

bench_header!(imf_fixdate, IfModifiedSince, { vec![b"Sun, 07 Nov 1994 08:48:37 GMT".to_vec()] });
bench_header!(rfc_850, IfModifiedSince, { vec![b"Sunday, 06-Nov-94 08:49:37 GMT".to_vec()] });
bench_header!(asctime, IfModifiedSince, { vec![b"Sun Nov  6 08:49:37 1994".to_vec()] });
