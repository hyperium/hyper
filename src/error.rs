//! Error and Result module.
use std::error::Error as StdError;
use std::fmt;
use std::io::Error as IoError;

use httparse;
use openssl::ssl::error::SslError;
use url;

use self::Error::{
    Method,
    Uri,
    Version,
    Header,
    Status,
    Io,
    Ssl,
    TooLarge
};


/// Result type often returned from methods that can have hyper `Error`s.
pub type Result<T> = ::std::result::Result<T, Error>;

/// A set of errors that can occur parsing HTTP streams.
#[derive(Debug)]
pub enum Error {
    /// An invalid `Method`, such as `GE,T`.
    Method,
    /// An invalid `RequestUri`, such as `exam ple.domain`.
    Uri(url::ParseError),
    /// An invalid `HttpVersion`, such as `HTP/1.1`
    Version,
    /// An invalid `Header`.
    Header,
    /// A message head is too large to be reasonable.
    TooLarge,
    /// An invalid `Status`, such as `1337 ELITE`.
    Status,
    /// An `io::Error` that occurred while trying to read or write to a network stream.
    Io(IoError),
    /// An error from the `openssl` library.
    Ssl(SslError)
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.description())
    }
}

impl StdError for Error {
    fn description(&self) -> &str {
        match *self {
            Method => "Invalid Method specified",
            Uri(_) => "Invalid Request URI specified",
            Version => "Invalid HTTP version specified",
            Header => "Invalid Header provided",
            TooLarge => "Message head is too large",
            Status => "Invalid Status provided",
            Io(ref e) => e.description(),
            Ssl(ref e) => e.description(),
        }
    }

    fn cause(&self) -> Option<&StdError> {
        match *self {
            Io(ref error) => Some(error),
            Ssl(ref error) => Some(error),
            Uri(ref error) => Some(error),
            _ => None,
        }
    }
}

impl From<IoError> for Error {
    fn from(err: IoError) -> Error {
        Io(err)
    }
}

impl From<url::ParseError> for Error {
    fn from(err: url::ParseError) -> Error {
        Uri(err)
    }
}

impl From<SslError> for Error {
    fn from(err: SslError) -> Error {
        match err {
            SslError::StreamError(err) => Io(err),
            err => Ssl(err),
        }
    }
}

impl From<httparse::Error> for Error {
    fn from(err: httparse::Error) -> Error {
        match err {
            httparse::Error::HeaderName => Header,
            httparse::Error::HeaderValue => Header,
            httparse::Error::NewLine => Header,
            httparse::Error::Status => Status,
            httparse::Error::Token => Header,
            httparse::Error::TooManyHeaders => TooLarge,
            httparse::Error::Version => Version,
        }
    }
}
