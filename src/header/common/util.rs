//! Utility functions for Header implementations.

use std::str::{FromStr, from_utf8};
use std::fmt::{mod, Show};
use time::{Tm, strptime};
use self::Encoding::{Chunked, Gzip, Deflate, Compress, EncodingExt};

/// Reads a single raw string when parsing a header
pub fn from_one_raw_str<T: FromStr>(raw: &[Vec<u8>]) -> Option<T> {
    if raw.len() != 1 {
        return None;
    }
    // we JUST checked that raw.len() == 1, so raw[0] WILL exist.
    match from_utf8(unsafe { raw[].unsafe_get(0)[] }) {
        Ok(s) => FromStr::from_str(s),
        Err(_) => None
    }
}

/// Reads a comma-delimited raw header into a Vec.
#[inline]
pub fn from_comma_delimited<T: FromStr>(raw: &[Vec<u8>]) -> Option<Vec<T>> {
    if raw.len() != 1 {
        return None;
    }
    // we JUST checked that raw.len() == 1, so raw[0] WILL exist.
    from_one_comma_delimited(unsafe { raw.as_slice().unsafe_get(0).as_slice() })
}

/// Reads a comma-delimited raw string into a Vec.
pub fn from_one_comma_delimited<T: FromStr>(raw: &[u8]) -> Option<Vec<T>> {
    match from_utf8(raw) {
        Ok(s) => {
            Some(s.as_slice()
                 .split([',', ' '].as_slice())
                 .filter_map(FromStr::from_str)
                 .collect())
        }
        Err(_) => None
    }
}

/// Format an array into a comma-delimited string.
pub fn fmt_comma_delimited<T: Show>(fmt: &mut fmt::Formatter, parts: &[T]) -> fmt::Result {
    let last = parts.len() - 1;
    for (i, part) in parts.iter().enumerate() {
        try!(part.fmt(fmt));
        if i < last {
            try!(", ".fmt(fmt));
        }
    }
    Ok(())
}

/// Get a Tm from HTTP date formats.
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
pub fn tm_from_str(s: &str) -> Option<Tm> {
    strptime(s, "%a, %d %b %Y %T %Z").or_else(|_| {
        strptime(s, "%A, %d-%b-%y %T %Z")
    }).or_else(|_| {
        strptime(s, "%c")
    }).ok()
}

/// A value to represent an encoding used in `Transfer-Encoding`
/// or `Accept-Encoding` header.
#[deriving(Clone, PartialEq)]
pub enum Encoding {
    /// The `chunked` encoding.
    Chunked,
    /// The `gzip` encoding.
    Gzip,
    /// The `deflate` encoding.
    Deflate,
    /// The `compress` encoding.
    Compress,
    /// Some other encoding that is less common, can be any String.
    EncodingExt(String)
}

impl fmt::Show for Encoding {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Chunked => "chunked",
            Gzip => "gzip",
            Deflate => "deflate",
            Compress => "compress",
            EncodingExt(ref s) => s.as_slice()
        }.fmt(fmt)
    }
}

impl FromStr for Encoding {
    fn from_str(s: &str) -> Option<Encoding> {
        match s {
            "chunked" => Some(Chunked),
            "deflate" => Some(Deflate),
            "gzip" => Some(Gzip),
            "compress" => Some(Compress),
            _ => Some(EncodingExt(s.to_string()))
        }
    }
}

/// Represents a quality value as defined in
/// [RFC7231](https://tools.ietf.org/html/rfc7231#section-5.3.1).
#[deriving(Clone, PartialEq)]
pub struct QualityValue<T> {
    value: T,
    quality: f32,
}

impl<T: fmt::Show> fmt::Show for QualityValue<T> {
    // TODO: Nicer formatting, currently e.g. quality 1 results in 1.000
    // but it is already standards conformant.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}; q={:.3}", self.value, self.quality)
    }
}

impl<T: FromStr> FromStr for QualityValue<T> {
    fn from_str(s: &str) -> Option<Self> {
        // Set defaults used if parsing fails.
        let mut raw_value = s;
        let mut quality = 1f32;

        let parts: Vec<&str> = s.rsplitn(1, ';').map(|x| x.trim()).collect();
        if parts.len() == 2 {
            let start = parts[0].slice(0, 2);
            if start == "q=" || start == "Q=" {
                let q_part = parts[0].slice(2, parts[0].len());
                if q_part.len() > 5 {
                    return None;
                }
                let x: Option<f32> = q_part.parse();
                match x {
                    Some(q_value) => {
                        if 0f32 <= q_value && q_value <= 1f32 {
                            quality = q_value;
                            raw_value = parts[1];
                        } else {
                            return None;
                        }
                    }
                    None => return None,
                }

            }
        }
        let x: Option<T> = raw_value.parse();
        match x {
            Some(value) => {
                Some(QualityValue{ value: value, quality: quality, })
            },
            None => return None,
        }
    }
}

#[test]
fn test_quality_value_show1() {
    // Most preferred
    let x = QualityValue{
        value: Chunked,
        quality: 1f32,
    };
    assert_eq!(format!("{}", x), "chunked; q=1.000");
}
#[test]
fn test_quality_value_show2() {
    // Least preferred
    let x = QualityValue{
        value: Chunked,
        quality: 0.001f32,
    };
    assert_eq!(format!("{}", x), "chunked; q=0.001");
}
#[test]
fn test_quality_value_show3() {
    // Custom value
    let x = QualityValue{
        value: EncodingExt("identity".to_string()),
        quality: 0.5f32,
    };
    assert_eq!(format!("{}", x), "identity; q=0.500");
}

#[test]
fn test_quality_value_from_str1() {
    let x: Option<QualityValue<Encoding>> = "chunked".parse();
    assert_eq!(x.unwrap(), QualityValue{ value: Chunked, quality: 1f32, });
}
#[test]
fn test_quality_value_from_str2() {
    let x: Option<QualityValue<Encoding>> = "chunked; q=1".parse();
    assert_eq!(x.unwrap(), QualityValue{ value: Chunked, quality: 1f32, });
}
#[test]
fn test_quality_value_from_str3() {
    let x: Option<QualityValue<Encoding>> = "gzip; q=0.5".parse();
    assert_eq!(x.unwrap(), QualityValue{ value: Gzip, quality: 0.5f32, });
}
#[test]
fn test_quality_value_from_str4() {
    let x: Option<QualityValue<Encoding>> = "gzip; q=0.273".parse();
    assert_eq!(x.unwrap(), QualityValue{ value: Gzip, quality: 0.273f32, });
}
#[test]
fn test_quality_value_from_str5() {
    let x: Option<QualityValue<Encoding>> = "gzip; q=0.2739999".parse();
    assert_eq!(x, None);
}
