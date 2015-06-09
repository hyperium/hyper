use std::fmt::{self, Display};
use std::str::FromStr;

use header::{Header, HeaderFormat, RangeUnit};
use header::parsing::{from_one_raw_str, from_one_comma_delimited};

/// `Range` header, defined in [RFC7233](https://tools.ietf.org/html/rfc7233#section-3.1)
///
/// The "Range" header field on a GET request modifies the method
/// semantics to request transfer of only one or more subranges of the
/// selected representation data, rather than the entire selected
/// representation data.
///
/// # ABNF
/// ```plain
/// Range =	byte-ranges-specifier / other-ranges-specifier
/// other-ranges-specifier = other-range-unit "=" other-range-set
/// other-range-set = 1*VCHAR
/// ```
///
/// # Example values
/// * `bytes=1000-`
/// * `bytes=-2000`
/// * `bytes=0-1,30-40`
/// * `custom_unit=0-123,-200`
///
/// # Examples
/// ```
/// use hyper::header::{Headers, Range, RangeSpec, RangeUnit};
///
/// let mut headers = Headers::new();
///
/// headers.set(Range {
///     unit: RangeUnit::Bytes,
///     ranges: vec![RangeSpec::FromTo(1, 100), RangeSpec::AllFrom(200)]
/// });
/// ```
/// ```
/// use hyper::header::{Headers, Range};
///
/// let mut headers = Headers::new();
/// headers.set(Range::bytes(1, 100));
/// ```
#[derive(PartialEq, Clone, Debug)]
pub struct Range {
    /// Unit of the Range i.e. bytes
    pub unit: RangeUnit,
    /// Set of ranges as defined in the HTTP spec
    pub ranges: Vec<RangeSpec>,
}

/// Each 'Range' header can contain one or more RangeSpecs.
/// Each RangeSpec defines a range of units to fetch
#[derive(PartialEq, Clone, Debug)]
pub enum RangeSpec {
    /// Get all bytes between x and y ("x-y")
    FromTo(u64, u64),
    /// Get all bytes starting from x ("x-")
    AllFrom(u64),
    /// Get last x bytes ("-x")
    Last(u64)
}

impl Range {
    /// Get the most common byte range header ("bytes=from-to")
    pub fn bytes(from: u64, to: u64) -> Range {
        Range {
            unit: RangeUnit::Bytes,
            ranges: vec![RangeSpec::FromTo(from, to)],
        }
    }
}


impl fmt::Display for RangeSpec {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            RangeSpec::FromTo(from, to) => write!(f, "{}-{}", from, to),
            RangeSpec::Last(pos) => write!(f, "-{}", pos),
            RangeSpec::AllFrom(pos) => write!(f, "{}-", pos),
        }
    }
}


impl fmt::Display for Range {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        try!(write!(f, "{}=", self.unit));

        for (i, range) in self.ranges.iter().enumerate() {
            if i != 0 {
                try!(f.write_str(","));
            }
            try!(Display::fmt(range, f));
        }
        Ok(())
    }
}

impl FromStr for Range {
    type Err = ::Error;

    fn from_str(s: &str) -> ::Result<Range> {
        let mut iter = s.splitn(2, "=");

        match (iter.next(), iter.next()) {
            (Some(unit), Some(ranges)) => {
                match (RangeUnit::from_str(unit), from_one_comma_delimited(ranges.as_bytes())) {
                    (Ok(unit), Ok(ranges)) => {
                        if ranges.is_empty() {
                            return Err(::Error::Header);
                        }
                        Ok(Range{unit: unit, ranges: ranges})
                    },
                    _ => Err(::Error::Header)
                }
            }
            _ => Err(::Error::Header)
        }
    }
}

impl FromStr for RangeSpec {
    type Err = ::Error;

    fn from_str(s: &str) -> ::Result<RangeSpec> {
        let mut parts = s.splitn(2, "-");

        match (parts.next(), parts.next()) {
            (Some(""), Some(end)) => {
                end.parse().or(Err(::Error::Header)).map(|end| RangeSpec::Last(end))
            },
            (Some(start), Some("")) => {
                start.parse().or(Err(::Error::Header)).map(|start| RangeSpec::AllFrom(start))
            },
            (Some(start), Some(end)) => {
                match (start.parse(), end.parse()) {
                    (Ok(start), Ok(end)) if start <= end => Ok(RangeSpec::FromTo(start, end)),
                    _ => Err(::Error::Header)
                }
            },
            _ => Err(::Error::Header)
        }
    }
}

impl Header for Range {

    fn header_name() -> &'static str {
        "Range"
    }

    fn parse_header(raw: &[Vec<u8>]) -> ::Result<Range> {
        from_one_raw_str(raw)
    }
}

impl HeaderFormat for Range {

    fn fmt_header(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(self, f)
    }

}

#[test]
fn test_parse_valid() {
    let r: Range = Header::parse_header(&[b"bytes=1-100".to_vec()]).unwrap();
    let r2: Range = Header::parse_header(&[b"bytes=1-100,-".to_vec()]).unwrap();
    let r3 =  Range::bytes(1, 100);
    assert_eq!(r, r2);
    assert_eq!(r2, r3);

    let r: Range = Header::parse_header(&[b"bytes=1-100,200-".to_vec()]).unwrap();
    let r2: Range = Header::parse_header(&[b"bytes= 1-100 , 101-xxx,  200- ".to_vec()]).unwrap();
    let r3 =  Range {
        unit: RangeUnit::Bytes,
        ranges: vec![RangeSpec::FromTo(1, 100), RangeSpec::AllFrom(200)]
    };
    assert_eq!(r, r2);
    assert_eq!(r2, r3);

    let r: Range = Header::parse_header(&[b"custom=1-100,-100".to_vec()]).unwrap();
    let r2: Range = Header::parse_header(&[b"custom=1-100, ,,-100".to_vec()]).unwrap();
    let r3 =  Range {
        unit: RangeUnit::Unregistered("custom".to_owned()),
        ranges: vec![RangeSpec::FromTo(1, 100), RangeSpec::Last(100)]
    };
    assert_eq!(r, r2);
    assert_eq!(r2, r3);
}

#[test]
fn test_parse_invalid() {
    let r: ::Result<Range> = Header::parse_header(&[b"bytes=1-a,-".to_vec()]);
    assert_eq!(r.ok(), None);

    let r: ::Result<Range> = Header::parse_header(&[b"bytes=1-2-3".to_vec()]);
    assert_eq!(r.ok(), None);

    let r: ::Result<Range> = Header::parse_header(&[b"abc".to_vec()]);
    assert_eq!(r.ok(), None);

    let r: ::Result<Range> = Header::parse_header(&[b"bytes=1-100=".to_vec()]);
    assert_eq!(r.ok(), None);

    let r: ::Result<Range> = Header::parse_header(&[b"bytes=".to_vec()]);
    assert_eq!(r.ok(), None);
}

#[test]
fn test_fmt() {
    use header::Headers;

    let range_header = Range {
        unit: RangeUnit::Bytes,
        ranges: vec![RangeSpec::FromTo(0, 1000), RangeSpec::AllFrom(2000)],
    };
    let mut headers = Headers::new();
    headers.set(range_header);

    assert_eq!(&headers.to_string(), "Range: bytes=0-1000,2000-\r\n");

    headers.clear();
    headers.set(Range {unit: RangeUnit::Bytes, ranges: vec![]});

    assert_eq!(&headers.to_string(), "Range: bytes=\r\n");
}

bench_header!(bytes_multi, Range, { vec![b"bytes=1-1001,2001-3001,10001-".to_vec()]});
bench_header!(custom_unit, Range, { vec![b"custom_unit=0-100000".to_vec()]});
