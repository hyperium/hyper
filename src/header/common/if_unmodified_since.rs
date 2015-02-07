use std::fmt;
use std::str::FromStr;
use time::Tm;
use header::{Header, HeaderFormat};
use header::parsing::from_one_raw_str;
use header::parsing::tm_from_str;

/// The `If-Unmodified-Since` header field.
#[derive(Copy, PartialEq, Clone, Debug)]
pub struct IfUnmodifiedSince(pub Tm);

deref!(IfUnmodifiedSince => Tm);

impl Header for IfUnmodifiedSince {
    fn header_name() -> &'static str {
        "If-Unmodified-Since"
    }

    fn parse_header(raw: &[Vec<u8>]) -> Option<IfUnmodifiedSince> {
        from_one_raw_str(raw)
    }
}


impl HeaderFormat for IfUnmodifiedSince {
    fn fmt_header(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let tm = self.0;
        let tm = match tm.tm_utcoff {
            0 => tm,
            _ => tm.to_utc(),
        };
        fmt::Display::fmt(&tm.rfc822(), fmt)
    }
}

impl FromStr for IfUnmodifiedSince {
    type Err = ();
    fn from_str(s: &str) -> Result<IfUnmodifiedSince, ()> {
        tm_from_str(s).map(IfUnmodifiedSince).ok_or(())
    }
}

bench_header!(imf_fixdate, IfUnmodifiedSince, { vec![b"Sun, 07 Nov 1994 08:48:37 GMT".to_vec()] });
bench_header!(rfc_850, IfUnmodifiedSince, { vec![b"Sunday, 06-Nov-94 08:49:37 GMT".to_vec()] });
bench_header!(asctime, IfUnmodifiedSince, { vec![b"Sun Nov  6 08:49:37 1994".to_vec()] });
