//! # hyper
#![feature(macro_rules, phase)]
#![warn(missing_doc)]
#![deny(warnings)]
#![experimental]

extern crate time;
extern crate url;
#[phase(plugin,link)] extern crate log;
#[cfg(test)] extern crate test;
extern crate "unsafe-any" as uany;

pub use std::io::net::ip::{SocketAddr, IpAddr, Ipv4Addr, Ipv6Addr, Port};
pub use mimewrapper::mime;
pub use url::Url;
pub use client::{get, head, post, delete, request};
pub use method::{Get, Head, Post, Delete};
pub use status::{Ok, BadRequest, NotFound};
pub use server::Server;

use std::fmt;
use std::io::IoError;

use std::rt::backtrace;


macro_rules! try_io(
    ($e:expr) => (match $e { Ok(v) => v, Err(e) => return Err(::HttpIoError(e)) })
)

macro_rules! todo(
    ($($arg:tt)*) => (if cfg!(not(ndebug)) {
        format_args!(|args| log!(5, "TODO: {}", args), $($arg)*)
    })
)

#[allow(dead_code)]
struct Trace;

impl fmt::Show for Trace {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let _ = backtrace::write(fmt);
        ::std::result::Ok(())
    }
}

macro_rules! trace(
    ($($arg:tt)*) => (if cfg!(not(ndebug)) {
        format_args!(|args| log!(5, "{}\n{}", args, ::Trace), $($arg)*)
    })
)

pub mod client;
pub mod method;
pub mod header;
pub mod server;
pub mod status;
pub mod uri;
pub mod version;

mod rfc7230;

mod mimewrapper {
    /// Re-exporting the mime crate, for convenience.
    extern crate mime;
}


/// Result type often returned from methods that can have `HttpError`s.
pub type HttpResult<T> = Result<T, HttpError>;

/// A set of errors that can occur parsing HTTP streams.
#[deriving(Show, PartialEq, Clone)]
pub enum HttpError {
    /// An invalid `Method`, such as `GE,T`.
    HttpMethodError,
    /// An invalid `RequestUri`, such as `exam ple.domain`.
    HttpUriError,
    /// An invalid `HttpVersion`, such as `HTP/1.1`
    HttpVersionError,
    /// An invalid `Header`.
    HttpHeaderError,
    /// An invalid `Status`, such as `1337 ELITE`.
    HttpStatusError,
    /// An `IoError` that occured while trying to read or write to a network stream.
    HttpIoError(IoError),
}

