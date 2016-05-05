use std::fmt::{self, Display};
use std::str::FromStr;

use header::{Header, HeaderFormat};
use header::parsing::{from_one_raw_str, from_comma_delimited};

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
///
/// bytes-unit = "bytes"
///
/// byte-ranges-specifier = bytes-unit "=" byte-range-set
/// byte-range-set = 1#(byte-range-spec / suffix-byte-range-spec)
/// byte-range-spec = first-byte-pos "-" [last-byte-pos]
/// first-byte-pos = 1*DIGIT
/// last-byte-pos = 1*DIGIT
/// ```
///
/// # Example values
/// * `bytes=1000-`
/// * `bytes=-2000`
/// * `bytes=0-1,30-40`
/// * `bytes=0-10,20-90,-100`
/// * `custom_unit=0-123`
/// * `custom_unit=xxx-yyy`
///
/// # Examples
/// ```
/// use hyper::header::{Headers, Range, ByteRangeSpec};
///
/// let mut headers = Headers::new();
/// headers.set(Range::Bytes(
///     vec![ByteRangeSpec::FromTo(1, 100), ByteRangeSpec::AllFrom(200)]
/// ));
///
/// headers.clear();
/// headers.set(Range::Unregistered("letters".to_owned(), "a-f".to_owned()));
/// ```
/// ```
/// use hyper::header::{Headers, Range};
///
/// let mut headers = Headers::new();
/// headers.set(Range::bytes(1, 100));
///
/// headers.clear();
/// headers.set(Range::bytes_multi(vec![(1, 100), (200, 300)]));
/// ```
#[derive(PartialEq, Clone, Debug)]
pub enum Range {
    /// Byte range
    Bytes(Vec<ByteRangeSpec>),
    /// Custom range, with unit not registered at IANA
    /// (`other-range-unit`: String , `other-range-set`: String)
    Unregistered(String, String)
}

/// Each `Range::Bytes` header can contain one or more `ByteRangeSpecs`.
/// Each `ByteRangeSpec` defines a range of bytes to fetch
#[derive(PartialEq, Clone, Debug)]
pub enum ByteRangeSpec {
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
        Range::Bytes(vec![ByteRangeSpec::FromTo(from, to)])
    }

    /// Get byte range header with multiple subranges
    /// ("bytes=from1-to1,from2-to2,fromX-toX")
    pub fn bytes_multi(ranges: Vec<(u64, u64)>) -> Range {
        Range::Bytes(ranges.iter().map(|r| ByteRangeSpec::FromTo(r.0, r.1)).collect())
    }
}


impl fmt::Display for ByteRangeSpec {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ByteRangeSpec::FromTo(from, to) => write!(f, "{}-{}", from, to),
            ByteRangeSpec::Last(pos) => write!(f, "-{}", pos),
            ByteRangeSpec::AllFrom(pos) => write!(f, "{}-", pos),
        }
    }
}


impl fmt::Display for Range {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Range::Bytes(ref ranges) => {
                try!(write!(f, "bytes="));

                for (i, range) in ranges.iter().enumerate() {
                    if i != 0 {
                        try!(f.write_str(","));
                    }
                    try!(Display::fmt(range, f));
                }
                Ok(())
            },
            Range::Unregistered(ref unit, ref range_str) => {
                write!(f, "{}={}", unit, range_str)
            },
        }
    }
}

impl FromStr for Range {
    type Err = ::Error;

    fn from_str(s: &str) -> ::Result<Range> {
        let mut iter = s.splitn(2, "=");

        match (iter.next(), iter.next()) {
            (Some("bytes"), Some(ranges)) => {
                match from_comma_delimited(&[ranges]) {
                    Ok(ranges) => {
                        if ranges.is_empty() {
                            return Err(::Error::Header);
                        }
                        Ok(Range::Bytes(ranges))
                    },
                    Err(_) => Err(::Error::Header)
                }
            }
            (Some(unit), Some(range_str)) if unit != "" && range_str != "" => {
                Ok(Range::Unregistered(unit.to_owned(), range_str.to_owned()))

            },
            _ => Err(::Error::Header)
        }
    }
}

impl FromStr for ByteRangeSpec {
    type Err = ::Error;

    fn from_str(s: &str) -> ::Result<ByteRangeSpec> {
        let mut parts = s.splitn(2, "-");

        match (parts.next(), parts.next()) {
            (Some(""), Some(end)) => {
                end.parse().or(Err(::Error::Header)).map(ByteRangeSpec::Last)
            },
            (Some(start), Some("")) => {
                start.parse().or(Err(::Error::Header)).map(ByteRangeSpec::AllFrom)
            },
            (Some(start), Some(end)) => {
                match (start.parse(), end.parse()) {
                    (Ok(start), Ok(end)) if start <= end => Ok(ByteRangeSpec::FromTo(start, end)),
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
fn test_parse_bytes_range_valid() {
    let r: Range = Header::parse_header(&[b"bytes=1-100".to_vec()]).unwrap();
    let r2: Range = Header::parse_header(&[b"bytes=1-100,-".to_vec()]).unwrap();
    let r3 =  Range::bytes(1, 100);
    assert_eq!(r, r2);
    assert_eq!(r2, r3);

    let r: Range = Header::parse_header(&[b"bytes=1-100,200-".to_vec()]).unwrap();
    let r2: Range = Header::parse_header(&[b"bytes= 1-100 , 101-xxx,  200- ".to_vec()]).unwrap();
    let r3 =  Range::Bytes(
        vec![ByteRangeSpec::FromTo(1, 100), ByteRangeSpec::AllFrom(200)]
    );
    assert_eq!(r, r2);
    assert_eq!(r2, r3);

    let r: Range = Header::parse_header(&[b"bytes=1-100,-100".to_vec()]).unwrap();
    let r2: Range = Header::parse_header(&[b"bytes=1-100, ,,-100".to_vec()]).unwrap();
    let r3 =  Range::Bytes(
        vec![ByteRangeSpec::FromTo(1, 100), ByteRangeSpec::Last(100)]
    );
    assert_eq!(r, r2);
    assert_eq!(r2, r3);

    let r: Range = Header::parse_header(&[b"custom=1-100,-100".to_vec()]).unwrap();
    let r2 =  Range::Unregistered("custom".to_owned(), "1-100,-100".to_owned());
    assert_eq!(r, r2);

}

#[test]
fn test_parse_unregistered_range_valid() {
    let r: Range = Header::parse_header(&[b"custom=1-100,-100".to_vec()]).unwrap();
    let r2 =  Range::Unregistered("custom".to_owned(), "1-100,-100".to_owned());
    assert_eq!(r, r2);

    let r: Range = Header::parse_header(&[b"custom=abcd".to_vec()]).unwrap();
    let r2 =  Range::Unregistered("custom".to_owned(), "abcd".to_owned());
    assert_eq!(r, r2);

    let r: Range = Header::parse_header(&[b"custom=xxx-yyy".to_vec()]).unwrap();
    let r2 =  Range::Unregistered("custom".to_owned(), "xxx-yyy".to_owned());
    assert_eq!(r, r2);
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

    let r: ::Result<Range> = Header::parse_header(&[b"custom=".to_vec()]);
    assert_eq!(r.ok(), None);

    let r: ::Result<Range> = Header::parse_header(&[b"=1-100".to_vec()]);
    assert_eq!(r.ok(), None);
}

#[test]
fn test_fmt() {
    use header::Headers;

    let mut headers = Headers::new();

    headers.set(
        Range::Bytes(
            vec![ByteRangeSpec::FromTo(0, 1000), ByteRangeSpec::AllFrom(2000)]
    ));
    assert_eq!(&headers.to_string(), "Range: bytes=0-1000,2000-\r\n");

    headers.clear();
    headers.set(Range::Bytes(vec![]));

    assert_eq!(&headers.to_string(), "Range: bytes=\r\n");

    headers.clear();
    headers.set(Range::Unregistered("custom".to_owned(), "1-xxx".to_owned()));

    assert_eq!(&headers.to_string(), "Range: custom=1-xxx\r\n");
}

bench_header!(bytes_multi, Range, { vec![b"bytes=1-1001,2001-3001,10001-".to_vec()]});
bench_header!(custom_unit, Range, { vec![b"other=0-100000".to_vec()]});
