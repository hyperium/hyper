use std::fmt;
use std::str::FromStr;
use time::Tm;
use header::{Header, HeaderFormat};
use header::parsing::from_one_raw_str;
use header::parsing::tm_from_str;

/// The `Expires` header field.
#[derive(Copy, PartialEq, Clone, Debug)]
pub struct Expires(pub Tm);

deref!(Expires => Tm);

impl Header for Expires {
    fn header_name() -> &'static str {
        "Expires"
    }

    fn parse_header(raw: &[Vec<u8>]) -> Option<Expires> {
        from_one_raw_str(raw)
    }
}


impl HeaderFormat for Expires {
    fn fmt_header(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let tm = self.0;
        let tm = match tm.tm_utcoff {
            0 => tm,
            _ => tm.to_utc(),
        };
        fmt::Display::fmt(&tm.rfc822(), fmt)
    }
}

impl FromStr for Expires {
    type Err = ();
    fn from_str(s: &str) -> Result<Expires, ()> {
        tm_from_str(s).map(Expires).ok_or(())
    }
}

bench_header!(imf_fixdate, Expires, { vec![b"Sun, 07 Nov 1994 08:48:37 GMT".to_vec()] });
bench_header!(rfc_850, Expires, { vec![b"Sunday, 06-Nov-94 08:49:37 GMT".to_vec()] });
bench_header!(asctime, Expires, { vec![b"Sun Nov  6 08:49:37 1994".to_vec()] });
