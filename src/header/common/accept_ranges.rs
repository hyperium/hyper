use std::fmt::{self, Display};
use std::str::FromStr;

header! {
    /// `Accept-Ranges` header, defined in
    /// [RFC7233](http://tools.ietf.org/html/rfc7233#section-2.3)
    ///
    /// The `Accept-Ranges` header field allows a server to indicate that it
    /// supports range requests for the target resource.
    ///
    /// # ABNF
    /// ```plain
    /// Accept-Ranges     = acceptable-ranges
    /// acceptable-ranges = 1#range-unit / \"none\"
    ///
    /// # Example values
    /// * `bytes`
    /// * `none`
    /// * `unknown-unit`
    /// ```
    ///
    /// # Examples
    /// ```
    /// use hyper::header::{Headers, AcceptRanges, RangeUnit};
    ///
    /// let mut headers = Headers::new();
    /// headers.set(AcceptRanges(vec![RangeUnit::Bytes]));
    /// ```
    /// ```
    /// use hyper::header::{Headers, AcceptRanges, RangeUnit};
    ///
    /// let mut headers = Headers::new();
    /// headers.set(AcceptRanges(vec![RangeUnit::None]));
    /// ```
    /// ```
    /// use hyper::header::{Headers, AcceptRanges, RangeUnit};
    ///
    /// let mut headers = Headers::new();
    /// headers.set(
    ///     AcceptRanges(vec![
    ///         RangeUnit::Unregistered("nibbles".to_owned()),
    ///         RangeUnit::Bytes,
    ///         RangeUnit::Unregistered("doublets".to_owned()),
    ///         RangeUnit::Unregistered("quadlets".to_owned()),
    ///     ])
    /// );
    /// ```
    (AcceptRanges, "Accept-Ranges") => (RangeUnit)+

    test_acccept_ranges {
        test_header!(test1, vec![b"bytes"]);
        test_header!(test2, vec![b"none"]);
        test_header!(test3, vec![b"unknown-unit"]);
        test_header!(test4, vec![b"bytes, unknown-unit"]);
    }
}

/// Range Units, described in [RFC7233](http://tools.ietf.org/html/rfc7233#section-2)
///
/// A representation can be partitioned into subranges according to
/// various structural units, depending on the structure inherent in the
/// representation's media type.
///
/// # ABNF
/// ```plain
/// range-unit       = bytes-unit / other-range-unit
/// bytes-unit       = "bytes"
/// other-range-unit = token
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RangeUnit {
    /// Indicating byte-range requests are supported.
    Bytes,
    /// Reserved as keyword, indicating no ranges are supported.
    None,
    /// The given range unit is not registered at IANA.
    Unregistered(String),
}


impl FromStr for RangeUnit {
    type Err = ::Error;
    fn from_str(s: &str) -> ::Result<Self> {
        match s {
            "bytes" => Ok(RangeUnit::Bytes),
            "none" => Ok(RangeUnit::None),
            // FIXME: Check if s is really a Token
            _ => Ok(RangeUnit::Unregistered(s.to_owned())),
        }
    }
}

impl Display for RangeUnit {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            RangeUnit::Bytes => f.write_str("bytes"),
            RangeUnit::None => f.write_str("none"),
            RangeUnit::Unregistered(ref x) => f.write_str(&x),
        }
    }
}
