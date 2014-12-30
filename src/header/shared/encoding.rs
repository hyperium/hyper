//! Provides an Encoding enum.

use std::fmt;
use std::str;

pub use self::Encoding::{Chunked, Gzip, Deflate, Compress, Identity, EncodingExt};

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
    /// The `identity` encoding.
    Identity,
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
            Identity => "identity",
            EncodingExt(ref s) => s.as_slice()
            }.fmt(fmt)
        }
    }

impl str::FromStr for Encoding {
    fn from_str(s: &str) -> Option<Encoding> {
        match s {
            "chunked" => Some(Chunked),
            "deflate" => Some(Deflate),
            "gzip" => Some(Gzip),
            "compress" => Some(Compress),
            "identity" => Some(Identity),
            _ => Some(EncodingExt(s.to_string()))
        }
    }
}
