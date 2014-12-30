//! Provides a struct for quality values.
//!
//! [RFC7231 Section 5.3.1](https://tools.ietf.org/html/rfc7231#section-5.3.1)
//! gives more information on quality values in HTTP header fields.

use std::fmt;
use std::str;
#[cfg(test)] use super::encoding::*;

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

impl<T: str::FromStr> str::FromStr for QualityValue<T> {
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
                        },
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
