#![feature(slicing_syntax, phase, macro_rules)]

extern crate url;
#[phase(plugin, link)] extern crate log;

use std::fmt;
use std::error::{Error, FromError};
use std::io::IoError;
use std::rt::backtrace;
use self::HttpError::{HttpMethodError, HttpUriError, HttpVersionError,
                      HttpHeaderError, HttpStatusError, HttpIoError};

#[macro_export] macro_rules! todo(
    ($($arg:tt)*) => (if cfg!(not(ndebug)) {
        format_args!(|args| log!(5, "TODO: {}", args), $($arg)*)
    })
)
#[macro_export] macro_rules! trace(
    ($($arg:tt)*) => (if cfg!(not(ndebug)) {
        format_args!(|args| log!(5, "{}\n{}", args, ::Trace), $($arg)*)
    })
)

#[macro_export] macro_rules! inspect(
    ($name:expr, $value:expr) => ({
        let v = $value;
        debug!("inspect: {} = {}", $name, v);
        v
    })
)

pub mod http;
pub mod method;
pub mod version;
pub mod status;
pub mod uri;

#[allow(dead_code)]
struct Trace;

impl fmt::Show for Trace {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let _ = backtrace::write(fmt);
        Result::Ok(())
    }
}

/// Result type often returned from methods that can have `HttpError`s.
pub type HttpResult<T> = Result<T, HttpError>;
/// A set of errors that can occur parsing HTTP streams.
#[deriving(Show, PartialEq, Clone)]
pub enum HttpError {
    /// An invalid `Method`, such as `GE,T`.
    HttpMethodError,
    /// An invalid `RequestUri`, such as `exam ple.domain`.
    HttpUriError(url::ParseError),
    /// An invalid `HttpVersion`, such as `HTP/1.1`
    HttpVersionError,
    /// An invalid `Header`.
    HttpHeaderError,
    /// An invalid `Status`, such as `1337 ELITE`.
    HttpStatusError,
    /// An `IoError` that occured while trying to read or write to a network stream.
    HttpIoError(IoError),
}

impl Error for HttpError {
    fn description(&self) -> &str {
        match *self {
            HttpMethodError => "Invalid Method specified",
            HttpUriError(_) => "Invalid Request URI specified",
            HttpVersionError => "Invalid HTTP version specified",
            HttpHeaderError => "Invalid Header provided",
            HttpStatusError => "Invalid Status provided",
            HttpIoError(_) => "An IoError occurred while connecting to the specified network",
        }
    }

    fn cause(&self) -> Option<&Error> {
        match *self {
            HttpIoError(ref error) => Some(error as &Error),
            HttpUriError(ref error) => Some(error as &Error),
            _ => None,
        }
    }
}

impl FromError<IoError> for HttpError {
    fn from_error(err: IoError) -> HttpError {
        HttpIoError(err)
    }
}

impl FromError<url::ParseError> for HttpError {
    fn from_error(err: url::ParseError) -> HttpError {
        HttpUriError(err)
    }
}
