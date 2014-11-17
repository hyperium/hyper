use header::{Header, HeaderFormat};
use std::fmt;
use std::str::FromStr;
use super::util::{from_comma_delimited, fmt_comma_delimited};

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
            EncodingExt(ref s) => s.as_slice()
        }.fmt(fmt)
    }
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
        from_comma_delimited(raw).map(|vec| TransferEncoding(vec))
    }
}

impl HeaderFormat for TransferEncoding {
    fn fmt_header(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let TransferEncoding(ref parts) = *self;
        fmt_comma_delimited(fmt, parts[])
    }
}

bench_header!(normal, TransferEncoding, { vec![b"chunked, gzip".to_vec()] })
bench_header!(ext, TransferEncoding, { vec![b"ext".to_vec()] })
