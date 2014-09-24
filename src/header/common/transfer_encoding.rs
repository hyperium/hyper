use header::Header;
use std::fmt::{mod, Show};
use std::from_str::FromStr;
use std::str::from_utf8;

/// The `Transfer-Encoding` header.
///
/// This header describes the encoding of the message body. It can be
/// comma-separated, including multiple encodings.
///
/// ```notrust
/// Transfer-Encoding: gzip, chunked
/// ```
///
/// According to the spec, if a `Content-Length` header is not included,
/// this header should include `chunked` as the last encoding.
///
/// The implementation uses a vector of `Encoding` values.
#[deriving(Clone, PartialEq, Show)]
pub struct TransferEncoding(pub Vec<Encoding>);

/// A value to be used with the `Transfer-Encoding` header.
///
/// Example:
///
/// ```
/// # use hyper::header::common::transfer_encoding::{TransferEncoding, Gzip, Chunked};
/// # use hyper::header::Headers;
/// # let mut headers = Headers::new();
/// headers.set(TransferEncoding(vec![Gzip, Chunked]));
#[deriving(Clone, PartialEq, Show)]
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

impl Header for TransferEncoding {
    fn header_name(_: Option<TransferEncoding>) -> &'static str {
        "Transfer-Encoding"
    }

    fn parse_header(raw: &[Vec<u8>]) -> Option<TransferEncoding> {
        if raw.len() != 1 {
            return None;
        }
        // we JUST checked that raw.len() == 1, so raw[0] WILL exist.
        match from_utf8(unsafe { raw.as_slice().unsafe_get(0).as_slice() }) {
            Some(s) => {
                Some(TransferEncoding(s.as_slice()
                     .split([',', ' '].as_slice())
                     .filter_map(from_str)
                     .collect()))
            }
            None => None
        }
    }

    fn fmt_header(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let TransferEncoding(ref parts) = *self;
        let last = parts.len() - 1;
        for (i, part) in parts.iter().enumerate() {
            try!(part.fmt(fmt));
            if i < last {
                try!(", ".fmt(fmt));
            }
        }
        Ok(())
    }
}

