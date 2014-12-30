//! Provides a struct for quality values.
//!
//! [RFC7231 Section 5.3.1](https://tools.ietf.org/html/rfc7231#section-5.3.1)
//! gives more information on quality values in HTTP header fields.

use std::fmt;
use std::str;
#[cfg(test)] use super::encoding::*;

/// Represents an item with a quality value as defined in
/// [RFC7231](https://tools.ietf.org/html/rfc7231#section-5.3.1).
#[deriving(Clone, PartialEq)]
pub struct QualityItem<T> {
    /// The actual contents of the field.
    pub item: T,
    /// The quality (client or server preference) for the value.
    pub quality: f32,
}

impl<T> QualityItem<T> {
    /// Creates a new `QualityItem` from an item and a quality.
    /// The item can be of any type.
    /// The quality should be a value in the range [0, 1].
    pub fn new(item: T, quality: f32) -> QualityItem<T> {
        QualityItem{item: item, quality: quality}
    }
}

impl<T: fmt::Show> fmt::Show for QualityItem<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}; q={}", self.item, format!("{:.3}", self.quality).trim_right_matches(['0', '.'].as_slice()))
    }
}

impl<T: str::FromStr> str::FromStr for QualityItem<T> {
    fn from_str(s: &str) -> Option<Self> {
        // Set defaults used if parsing fails.
        let mut raw_item = s;
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
                            raw_item = parts[1];
                            } else {
                                return None;
                            }
                        },
                    None => return None,
                }
            }
        }
        let x: Option<T> = raw_item.parse();
        match x {
            Some(item) => {
                Some(QualityItem{ item: item, quality: quality, })
            },
            None => return None,
        }
    }
}

/// Convinience function to wrap a value in a `QualityItem`
/// Sets `q` to the default 1.0
pub fn qitem<T>(item: T) -> QualityItem<T> {
    QualityItem::new(item, 1.0)
}

#[test]
fn test_quality_item_show1() {
    let x = qitem(Chunked);
    assert_eq!(format!("{}", x), "chunked; q=1.000");
}
#[test]
fn test_quality_item_show2() {
    let x = QualityItem::new(Chunked, 0.001);
    assert_eq!(format!("{}", x), "chunked; q=0.001");
}
#[test]
fn test_quality_item_show3() {
    // Custom value
    let x = QualityItem{
        item: EncodingExt("identity".to_string()),
        quality: 0.5f32,
    };
    assert_eq!(format!("{}", x), "identity; q=0.500");
}

#[test]
fn test_quality_item_from_str1() {
    let x: Option<QualityItem<Encoding>> = "chunked".parse();
    assert_eq!(x.unwrap(), QualityItem{ item: Chunked, quality: 1f32, });
}
#[test]
fn test_quality_item_from_str2() {
    let x: Option<QualityItem<Encoding>> = "chunked; q=1".parse();
    assert_eq!(x.unwrap(), QualityItem{ item: Chunked, quality: 1f32, });
}
#[test]
fn test_quality_item_from_str3() {
    let x: Option<QualityItem<Encoding>> = "gzip; q=0.5".parse();
    assert_eq!(x.unwrap(), QualityItem{ item: Gzip, quality: 0.5f32, });
}
#[test]
fn test_quality_item_from_str4() {
    let x: Option<QualityItem<Encoding>> = "gzip; q=0.273".parse();
    assert_eq!(x.unwrap(), QualityItem{ item: Gzip, quality: 0.273f32, });
}
#[test]
fn test_quality_item_from_str5() {
    let x: Option<QualityItem<Encoding>> = "gzip; q=0.2739999".parse();
    assert_eq!(x, None);
}
