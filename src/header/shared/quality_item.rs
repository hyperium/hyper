use std::cmp;
use std::default::Default;
use std::fmt;
use std::str;

/// Represents a quality used in quality values.
///
/// Can be created with the `q` function.
///
/// # Implementation notes
///
/// The quality value is defined as a number between 0 and 1 with three decimal places. This means
/// there are 1000 possible values. Since floating point numbers are not exact and the smallest
/// floating point data type (`f32`) consumes four bytes, hyper uses an `u16` value to store the
/// quality internally. For performance reasons you may set quality directly to a value between
/// 0 and 1000 e.g. `Quality(532)` matches the quality `q=0.532`.
///
/// [RFC7231 Section 5.3.1](https://tools.ietf.org/html/rfc7231#section-5.3.1)
/// gives more information on quality values in HTTP header fields.
#[derive(Copy, Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Quality(pub u16);

impl fmt::Display for Quality {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.0 {
            1000 => Ok(()),
            0 => f.write_str("; q=0"),
            x => write!(f, "; q=0.{}", format!("{:03}", x).trim_right_matches('0'))
        }
    }
}

impl Default for Quality {
    fn default() -> Quality {
        Quality(1000)
    }
}

/// Represents an item with a quality value as defined in
/// [RFC7231](https://tools.ietf.org/html/rfc7231#section-5.3.1).
#[derive(Clone, PartialEq, Debug)]
pub struct QualityItem<T> {
    /// The actual contents of the field.
    pub item: T,
    /// The quality (client or server preference) for the value.
    pub quality: Quality,
}

impl<T> QualityItem<T> {
    /// Creates a new `QualityItem` from an item and a quality.
    /// The item can be of any type.
    /// The quality should be a value in the range [0, 1].
    pub fn new(item: T, quality: Quality) -> QualityItem<T> {
        QualityItem {
            item: item,
            quality: quality
        }
    }
}

impl<T: PartialEq> cmp::PartialOrd for QualityItem<T> {
    fn partial_cmp(&self, other: &QualityItem<T>) -> Option<cmp::Ordering> {
        self.quality.partial_cmp(&other.quality)
    }
}

impl<T: fmt::Display> fmt::Display for QualityItem<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}{}", self.item, format!("{}", self.quality))
    }
}

impl<T: str::FromStr> str::FromStr for QualityItem<T> {
    type Err = ::Error;
    fn from_str(s: &str) -> ::Result<QualityItem<T>> {
        // Set defaults used if parsing fails.
        let mut raw_item = s;
        let mut quality = 1f32;

        let parts: Vec<&str> = s.rsplitn(2, ';').map(|x| x.trim()).collect();
        if parts.len() == 2 {
            let start = &parts[0][0..2];
            if start == "q=" || start == "Q=" {
                let q_part = &parts[0][2..parts[0].len()];
                if q_part.len() > 5 {
                    return Err(::Error::Header);
                }
                match q_part.parse::<f32>() {
                    Ok(q_value) => {
                        if 0f32 <= q_value && q_value <= 1f32 {
                            quality = q_value;
                            raw_item = parts[1];
                            } else {
                                return Err(::Error::Header);
                            }
                        },
                    Err(_) => return Err(::Error::Header),
                }
            }
        }
        match raw_item.parse::<T>() {
            // we already checked above that the quality is within range
            Ok(item) => Ok(QualityItem::new(item, from_f32(quality))),
            Err(_) => return Err(::Error::Header),
        }
    }
}

fn from_f32(f: f32) -> Quality {
    // this function is only used internally. A check that `f` is within range
    // should be done before calling this method. Just in case, this
    // debug_assert should catch if we were forgetful
    debug_assert!(f >= 0f32 && f <= 1f32, "q value must be between 0.0 and 1.0");
    Quality((f * 1000f32) as u16)
}

/// Convinience function to wrap a value in a `QualityItem`
/// Sets `q` to the default 1.0
pub fn qitem<T>(item: T) -> QualityItem<T> {
    QualityItem::new(item, Default::default())
}

/// Convenience function to create a `Quality` fromt a float.
pub fn q(f: f32) -> Quality {
    assert!(f >= 0f32 && f <= 1f32, "q value must be between 0.0 and 1.0");
    from_f32(f)
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::encoding::*;

    #[test]
    fn test_quality_item_show1() {
        let x = qitem(Chunked);
        assert_eq!(format!("{}", x), "chunked");
    }
    #[test]
    fn test_quality_item_show2() {
        let x = QualityItem::new(Chunked, Quality(1));
        assert_eq!(format!("{}", x), "chunked; q=0.001");
    }
    #[test]
    fn test_quality_item_show3() {
        // Custom value
        let x = QualityItem{
            item: EncodingExt("identity".to_owned()),
            quality: Quality(500),
        };
        assert_eq!(format!("{}", x), "identity; q=0.5");
    }

    #[test]
    fn test_quality_item_from_str1() {
        let x: ::Result<QualityItem<Encoding>> = "chunked".parse();
        assert_eq!(x.unwrap(), QualityItem{ item: Chunked, quality: Quality(1000), });
    }
    #[test]
    fn test_quality_item_from_str2() {
        let x: ::Result<QualityItem<Encoding>> = "chunked; q=1".parse();
        assert_eq!(x.unwrap(), QualityItem{ item: Chunked, quality: Quality(1000), });
    }
    #[test]
    fn test_quality_item_from_str3() {
        let x: ::Result<QualityItem<Encoding>> = "gzip; q=0.5".parse();
        assert_eq!(x.unwrap(), QualityItem{ item: Gzip, quality: Quality(500), });
    }
    #[test]
    fn test_quality_item_from_str4() {
        let x: ::Result<QualityItem<Encoding>> = "gzip; q=0.273".parse();
        assert_eq!(x.unwrap(), QualityItem{ item: Gzip, quality: Quality(273), });
    }
    #[test]
    fn test_quality_item_from_str5() {
        let x: ::Result<QualityItem<Encoding>> = "gzip; q=0.2739999".parse();
        assert!(x.is_err());
    }
    #[test]
    fn test_quality_item_from_str6() {
        let x: ::Result<QualityItem<Encoding>> = "gzip; q=2".parse();
        assert!(x.is_err());
    }
    #[test]
    fn test_quality_item_ordering() {
        let x: QualityItem<Encoding> = "gzip; q=0.5".parse().ok().unwrap();
        let y: QualityItem<Encoding> = "gzip; q=0.273".parse().ok().unwrap();
        let comparision_result: bool = x.gt(&y);
        assert!(comparision_result)
    }

    #[test]
    fn test_quality() {
        assert_eq!(q(0.5), Quality(500));
    }

    #[test]
    fn test_quality2() {
        assert_eq!(format!("{}", q(0.0)), "; q=0");
    }

    #[test]
    #[should_panic]
    fn test_quality_invalid() {
        q(-1.0);
    }

    #[test]
    #[should_panic]
    fn test_quality_invalid2() {
        q(2.0);
    }
}
