//! Utility functions for Header implementations.

use std::str::{FromStr, from_utf8};
use std::fmt::{mod, Show};

/// Reads a single raw string when parsing a header
pub fn from_one_raw_str<T: FromStr>(raw: &[Vec<u8>]) -> Option<T> {
    if raw.len() != 1 {
        return None;
    }
    // we JUST checked that raw.len() == 1, so raw[0] WILL exist.
    match from_utf8(unsafe { raw[].unsafe_get(0)[] }) {
        Some(s) => FromStr::from_str(s),
        None => None
    }
}

/// Reads a comma-delimited raw string into a Vec.
pub fn from_comma_delimited<T: FromStr>(raw: &[Vec<u8>]) -> Option<Vec<T>> {
    if raw.len() != 1 {
        return None;
    }
    // we JUST checked that raw.len() == 1, so raw[0] WILL exist.
    match from_utf8(unsafe { raw.as_slice().unsafe_get(0).as_slice() }) {
        Some(s) => {
            Some(s.as_slice()
                 .split([',', ' '].as_slice())
                 .filter_map(from_str)
                 .collect())
        }
        None => None
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
