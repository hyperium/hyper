use header::{Header, HeaderFormat};
use std::fmt::{mod, Show};
use super::util::from_one_raw_str;
use std::str::FromStr;
use time::{Tm, strptime};

// Egh, replace as soon as something better than time::Tm exists.
/// The `Date` header field.
#[deriving(PartialEq, Clone)]
pub struct Date(pub Tm);

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
        self.fmt(fmt)
    }
}

impl fmt::Show for Date {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let Date(ref tm) = *self;
        // bummer that tm.strftime allocates a string. It would nice if it
        // returned a Show instead, since I don't need the String here
        match tm.tm_utcoff {
            0 => tm.rfc822().fmt(fmt),
            _ => tm.to_utc().rfc822().fmt(fmt)
        }
    }
}

impl FromStr for Date {
    //    Prior to 1995, there were three different formats commonly used by
    //   servers to communicate timestamps.  For compatibility with old
    //   implementations, all three are defined here.  The preferred format is
    //   a fixed-length and single-zone subset of the date and time
    //   specification used by the Internet Message Format [RFC5322].
    //
    //     HTTP-date    = IMF-fixdate / obs-date
    //
    //   An example of the preferred format is
    //
    //     Sun, 06 Nov 1994 08:49:37 GMT    ; IMF-fixdate
    //
    //   Examples of the two obsolete formats are
    //
    //     Sunday, 06-Nov-94 08:49:37 GMT   ; obsolete RFC 850 format
    //     Sun Nov  6 08:49:37 1994         ; ANSI C's asctime() format
    //
    //   A recipient that parses a timestamp value in an HTTP header field
    //   MUST accept all three HTTP-date formats.  When a sender generates a
    //   header field that contains one or more timestamps defined as
    //   HTTP-date, the sender MUST generate those timestamps in the
    //   IMF-fixdate format.
    fn from_str(s: &str) -> Option<Date> {
        strptime(s, "%a, %d %b %Y %T %Z").or_else(|_| {
            strptime(s, "%A, %d-%b-%y %T %Z")
        }).or_else(|_| {
            strptime(s, "%c")
        }).ok().map(|tm| Date(tm))
    }
}

bench_header!(imf_fixdate, Date, { vec![b"Sun, 07 Nov 1994 08:48:37 GMT".to_vec()] })
bench_header!(rfc_850, Date, { vec![b"Sunday, 06-Nov-94 08:49:37 GMT".to_vec()] })
bench_header!(asctime, Date, { vec![b"Sun Nov  6 08:49:37 1994".to_vec()] })
