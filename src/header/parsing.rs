//! Utility functions for Header implementations.

use std::str;
use std::fmt::{self, Display};

/// Reads a single raw string when parsing a header
pub fn from_one_raw_str<T: str::FromStr>(raw: &[Vec<u8>]) -> ::Result<T> {
    if raw.len() != 1 || unsafe { raw.get_unchecked(0) } == b"" { return Err(::Error::Header) }
    // we JUST checked that raw.len() == 1, so raw[0] WILL exist.
    let s: &str = try!(str::from_utf8(& unsafe { raw.get_unchecked(0) }[..]));
    if let Ok(x) = str::FromStr::from_str(s) {
        Ok(x)
    } else {
        Err(::Error::Header)
    }
}

/// Reads a comma-delimited raw header into a Vec.
#[inline]
pub fn from_comma_delimited<T: str::FromStr>(raw: &[Vec<u8>]) -> ::Result<Vec<T>> {
    if raw.len() != 1 {
        return Err(::Error::Header);
    }
    // we JUST checked that raw.len() == 1, so raw[0] WILL exist.
    from_one_comma_delimited(& unsafe { raw.get_unchecked(0) }[..])
}

/// Reads a comma-delimited raw string into a Vec.
pub fn from_one_comma_delimited<T: str::FromStr>(raw: &[u8]) -> ::Result<Vec<T>> {
    let s = try!(str::from_utf8(raw));
    Ok(s.split(',')
        .filter_map(|x| match x.trim() {
            "" => None,
            y => Some(y)
        })
        .filter_map(|x| x.parse().ok())
        .collect())
}

/// Format an array into a comma-delimited string.
pub fn fmt_comma_delimited<T: Display>(f: &mut fmt::Formatter, parts: &[T]) -> fmt::Result {
    for (i, part) in parts.iter().enumerate() {
        if i != 0 {
            try!(f.write_str(", "));
        }
        try!(Display::fmt(part, f));
    }
    Ok(())
}
