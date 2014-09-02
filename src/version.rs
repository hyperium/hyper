//! # HTTP Versions
//!
//! Instead of relying on typo-prone Strings, use expected HTTP versions as
//! the `HttpVersion` enum.
use std::fmt;

/// Represents a version of the HTTP spec.
#[deriving(PartialEq, PartialOrd)]
pub enum HttpVersion {
    /// `HTTP/0.9`
    Http09,
    /// `HTTP/1.0`
    Http10,
    /// `HTTP/1.1`
    Http11,
    /// `HTTP/2.0`
    Http20
}

impl fmt::Show for HttpVersion {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Http09 => "HTTP/0.9",
            Http10 => "HTTP/1.0",
            Http11 => "HTTP/1.1",
            Http20 => "HTTP/2.0",
        }.fmt(fmt)
    }
}
