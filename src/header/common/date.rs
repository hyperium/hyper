use std::fmt::{mod, Show};
use std::str::FromStr;
use time::Tm;
use header::{Header, HeaderFormat};
use super::util::{from_one_raw_str, tm_from_str};

// Egh, replace as soon as something better than time::Tm exists.
/// The `Date` header field.
#[deriving(Copy, PartialEq, Clone)]
pub struct Date(pub Tm);

deref!(Date -> Tm)

impl Header for Date {
    fn header_name(_: Option<Date>) -> &'static str {
        "Date"
    }

    fn parse_header(raw: &[Vec<u8>]) -> Option<Date> {
        from_one_raw_str(raw)
    }
}


impl HeaderFormat for Date {
    fn fmt_header(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let tm = **self;
        match tm.tm_utcoff {
            0 => tm.rfc822().fmt(fmt),
            _ => tm.to_utc().rfc822().fmt(fmt)
        }
    }
}

impl FromStr for Date {
    fn from_str(s: &str) -> Option<Date> {
        tm_from_str(s).map(Date)
    }
}

bench_header!(imf_fixdate, Date, { vec![b"Sun, 07 Nov 1994 08:48:37 GMT".to_vec()] })
bench_header!(rfc_850, Date, { vec![b"Sunday, 06-Nov-94 08:49:37 GMT".to_vec()] })
bench_header!(asctime, Date, { vec![b"Sun Nov  6 08:49:37 1994".to_vec()] })
