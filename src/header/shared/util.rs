//! Utility functions for Header implementations.

use std::str;
use std::fmt;

/// Reads a single raw string when parsing a header
pub fn from_one_raw_str<T: str::FromStr>(raw: &[Vec<u8>]) -> Option<T> {
    if raw.len() != 1 {
        return None;
    }
    // we JUST checked that raw.len() == 1, so raw[0] WILL exist.
    match str::from_utf8(unsafe { raw[].unsafe_get(0)[] }) {
        Ok(s) => str::FromStr::from_str(s),
        Err(_) => None
    }
}

/// Reads a comma-delimited raw header into a Vec.
#[inline]
pub fn from_comma_delimited<T: str::FromStr>(raw: &[Vec<u8>]) -> Option<Vec<T>> {
    if raw.len() != 1 {
        return None;
    }
    // we JUST checked that raw.len() == 1, so raw[0] WILL exist.
    from_one_comma_delimited(unsafe { raw.as_slice().unsafe_get(0).as_slice() })
}

/// Reads a comma-delimited raw string into a Vec.
pub fn from_one_comma_delimited<T: str::FromStr>(raw: &[u8]) -> Option<Vec<T>> {
    match str::from_utf8(raw) {
        Ok(s) => {
            Some(s.as_slice()
                 .split(',')
                 .map(|x| x.trim())
                 .filter_map(str::FromStr::from_str)
                 .collect())
        }
        Err(_) => None
    }
}

/// Format an array into a comma-delimited string.
pub fn fmt_comma_delimited<T: fmt::Show>(fmt: &mut fmt::Formatter, parts: &[T]) -> fmt::Result {
    let last = parts.len() - 1;
    for (i, part) in parts.iter().enumerate() {
        try!(write!(fmt, "{}", part));
        if i < last {
            try!(write!(fmt, ", "));
        }
    }
    Ok(())
}
