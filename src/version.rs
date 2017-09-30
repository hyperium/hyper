//! HTTP Versions enum
//!
//! Instead of relying on typo-prone Strings, use expected HTTP versions as
//! the `HttpVersion` enum.
use std::fmt;
use std::str::FromStr;

#[cfg(feature = "compat")]
use http;

use error::Error;
use self::HttpVersion::{Http09, Http10, Http11, H2, H2c};

/// Represents a version of the HTTP spec.
#[derive(PartialEq, PartialOrd, Copy, Clone, Eq, Ord, Hash, Debug)]
pub enum HttpVersion {
    /// `HTTP/0.9`
    Http09,
    /// `HTTP/1.0`
    Http10,
    /// `HTTP/1.1`
    Http11,
    /// `HTTP/2.0` over TLS
    H2,
    /// `HTTP/2.0` over cleartext
    H2c,
    #[doc(hidden)]
    __DontMatchMe,
}

impl fmt::Display for HttpVersion {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.write_str(match *self {
            Http09 => "HTTP/0.9",
            Http10 => "HTTP/1.0",
            Http11 => "HTTP/1.1",
            H2 => "h2",
            H2c => "h2c",
            HttpVersion::__DontMatchMe => unreachable!(),
        })
    }
}

impl FromStr for HttpVersion {
    type Err = Error;
    fn from_str(s: &str) -> Result<HttpVersion, Error> {
        Ok(match s {
            "HTTP/0.9" => Http09,
            "HTTP/1.0" => Http10,
            "HTTP/1.1" => Http11,
            "h2" => H2,
            "h2c" => H2c,
            _ => return Err(Error::Version),
        })
    }
}

impl Default for HttpVersion {
    fn default() -> HttpVersion {
        Http11
    }
}

#[cfg(feature = "compat")]
impl From<http::Version> for HttpVersion {
    fn from(v: http::Version) -> HttpVersion {
        match v {
            http::Version::HTTP_09 =>
                HttpVersion::Http09,
            http::Version::HTTP_10 =>
                HttpVersion::Http10,
            http::Version::HTTP_11 =>
                HttpVersion::Http11,
            http::Version::HTTP_2 =>
                HttpVersion::H2
        }
    }
}

#[cfg(feature = "compat")]
impl From<HttpVersion> for http::Version {
    fn from(v: HttpVersion) -> http::Version {
        match v {
            HttpVersion::Http09 =>
                http::Version::HTTP_09,
            HttpVersion::Http10 =>
                http::Version::HTTP_10,
            HttpVersion::Http11 =>
                http::Version::HTTP_11,
            HttpVersion::H2 =>
                http::Version::HTTP_2,
            _ => panic!("attempted to convert unexpected http version")
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;
    use error::Error;
    use super::HttpVersion;
    use super::HttpVersion::{Http09,Http10,Http11,H2,H2c};

    #[test]
    fn test_default() {
        assert_eq!(Http11, HttpVersion::default());
    }

    #[test]
    fn test_from_str() {
        assert_eq!(Http09, HttpVersion::from_str("HTTP/0.9").unwrap());
        assert_eq!(Http10, HttpVersion::from_str("HTTP/1.0").unwrap());
        assert_eq!(Http11, HttpVersion::from_str("HTTP/1.1").unwrap());
        assert_eq!(H2, HttpVersion::from_str("h2").unwrap());
        assert_eq!(H2c, HttpVersion::from_str("h2c").unwrap());
    }

    #[test]
    fn test_from_str_panic() {
        match HttpVersion::from_str("foo") {
            Err(Error::Version) => assert!(true),
            Err(_) => assert!(false),
            Ok(_) => assert!(false),
        }
    }
        
}
