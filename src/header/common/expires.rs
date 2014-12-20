use std::fmt::{mod, Show};
use std::str::FromStr;
use time::Tm;
use header::{Header, HeaderFormat};
use super::util::{from_one_raw_str, tm_from_str};

/// The `Expires` header field.
#[deriving(Copy, PartialEq, Clone)]
pub struct Expires(pub Tm);

deref!(Expires -> Tm);

impl Header for Expires {
    fn header_name(_: Option<Expires>) -> &'static str {
        "Expires"
    }

    fn parse_header(raw: &[Vec<u8>]) -> Option<Expires> {
        from_one_raw_str(raw)
    }
}


impl HeaderFormat for Expires {
    fn fmt_header(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let tm = **self;
        match tm.tm_utcoff {
            0 => tm.rfc822().fmt(fmt),
            _ => tm.to_utc().rfc822().fmt(fmt)
        }
    }
}

impl FromStr for Expires {
    fn from_str(s: &str) -> Option<Expires> {
        tm_from_str(s).map(Expires)
    }
}

bench_header!(imf_fixdate, Expires, { vec![b"Sun, 07 Nov 1994 08:48:37 GMT".to_vec()] });
bench_header!(rfc_850, Expires, { vec![b"Sunday, 06-Nov-94 08:49:37 GMT".to_vec()] });
bench_header!(asctime, Expires, { vec![b"Sun Nov  6 08:49:37 1994".to_vec()] });

