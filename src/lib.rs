#![feature(macro_rules, phase, default_type_params, if_let, slicing_syntax,
           tuple_indexing)]
#![deny(missing_docs)]
#![deny(warnings)]
#![experimental]

//! # Hyper
//! Hyper is a fast, modern HTTP implementation written in and for Rust. It
//! is a low-level typesafe abstraction over raw HTTP, providing an elegant
//! layer over "stringly-typed" HTTP.
//!
//! Hyper offers both an HTTP/S client an HTTP server which can be used to drive
//! complex web applications written entirely in Rust.
//!
//! ## Internal Design
//!
//! Hyper is designed as a relatively low-level wrapped over raw HTTP. It should
//! allow the implementation of higher-level abstractions with as little pain as
//! possible, and should not irrevocably hide any information from its users.
//!
//! ### Common Functionality
//!
//! Functionality and code shared between the Server and Client implementations can
//! be found in `src` directly - this includes `NetworkStream`s, `Method`s,
//! `StatusCode`, and so on.
//!
//! #### Methods
//!
//! Methods are represented as a single `enum` to remain as simple as possible.
//! Extension Methods are represented as raw `String`s. A method's safety and
//! idempotence can be accessed using the `safe` and `idempotent` methods.
//!
//! #### StatusCode
//!
//! Status codes are also represented as a single, exhaustive, `enum`. This
//! representation is efficient, typesafe, and ergonomic as it allows the use of
//! `match` to disambiguate known status codes.
//!
//! #### Headers
//!
//! Hyper's header representation is likely the most complex API exposed by Hyper.
//!
//! Hyper's headers are an abstraction over an internal `HashMap` and provides a
//! typesafe API for interacting with headers that does not rely on the use of
//! "string-typing."
//!
//! Each HTTP header in Hyper has an associated type and implementation of the
//! `Header` trait, which defines an HTTP headers name as a string, how to parse
//! that header, and how to format that header.
//!
//! Headers are then parsed from the string representation lazily when the typed
//! representation of a header is requested and formatted back into their string
//! representation when headers are written back to the client.
//!
//! #### NetworkStream and NetworkAcceptor
//!
//! These are found in `src/net.rs` and define the interface that acceptors and
//! streams must fulfill for them to be used within Hyper. They are by and large
//! internal tools and you should only need to mess around with them if you want to
//! mock or replace `TcpStream` and `TcpAcceptor`.
//!
//! ### Server
//!
//! Server-specific functionality, such as `Request` and `Response`
//! representations, are found in in `src/server`.
//!
//! #### Handler + Server
//!
//! A Handler in Hyper just accepts an Iterator of `(Request, Response)` pairs and
//! does whatever it wants with it. This gives Handlers maximum flexibility to decide
//! on concurrency strategy and exactly how they want to distribute the work of
//! dealing with `Request` and `Response.`
//!
//! #### Request
//!
//! An incoming HTTP Request is represented as a struct containing
//! a `Reader` over a `NetworkStream`, which represents the body, headers, a remote
//! address, an HTTP version, and a `Method` - relatively standard stuff.
//!
//! `Request` implements `Reader` itself, meaning that you can ergonomically get
//! the body out of a `Request` using standard `Reader` methods and helpers.
//!
//! #### Response
//!
//! An outgoing HTTP Response is also represented as a struct containing a `Writer`
//! over a `NetworkStream` which represents the Response body in addition to
//! standard items such as the `StatusCode` and HTTP version. `Response`'s `Writer`
//! implementation provides a streaming interface for sending data over to the
//! client.
//!
//! One of the traditional problems with representing outgoing HTTP Responses is
//! tracking the write-status of the Response - have we written the status-line,
//! the headers, the body, etc.? Hyper tracks this information statically using the
//! type system and prevents you, using the type system, from writing headers after
//! you have started writing to the body or vice versa.
//!
//! Hyper does this through a phantom type parameter in the definition of Response,
//! which tracks whether you are allowed to write to the headers or the body. This
//! phantom type can have two values `Fresh` or `Streaming`, with `Fresh`
//! indicating that you can write the headers and `Streaming` indicating that you
//! may write to the body, but not the headers.
//!
//! ### Client
//!
//! Client-specific functionality, such as `Request` and `Response`
//! representations, are found in `src/client`.
//!
//! #### Request
//!
//! An outgoing HTTP Request is represented as a struct containing a `Writer` over
//! a `NetworkStream` which represents the Request body in addition to the standard
//! information such as headers and the request method.
//!
//! Outgoing Requests track their write-status in almost exactly the same way as
//! outgoing HTTP Responses do on the Server, so we will defer to the explanation
//! in the documentation for sever Response.
//!
//! Requests expose an efficient streaming interface instead of a builder pattern,
//! but they also provide the needed interface for creating a builder pattern over
//! the API exposed by core Hyper.
//!
//! #### Response
//!
//! Incoming HTTP Responses are represented as a struct containing a `Reader` over
//! a `NetworkStream` and contain headers, a status, and an http version. They
//! implement `Reader` and can be read to get the data out of a `Response`.
//!

extern crate serialize;
extern crate time;
extern crate url;
extern crate openssl;
#[phase(plugin,link)] extern crate log;
#[cfg(test)] extern crate test;
extern crate "unsafe-any" as uany;
extern crate "move-acceptor" as macceptor;
extern crate intertwine;
extern crate typeable;
extern crate cookie;

pub use std::io::net::ip::{SocketAddr, IpAddr, Ipv4Addr, Ipv6Addr, Port};
pub use mimewrapper::mime;
pub use url::Url;
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

macro_rules! inspect(
    ($name:expr, $value:expr) => ({
        let v = $value;
        debug!("inspect: $name = {}", v);
        v
    })
)

pub mod client;
pub mod method;
pub mod header;
pub mod http;
pub mod net;
pub mod server;
pub mod status;
pub mod uri;
pub mod version;

#[cfg(test)] mod mock;

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

//FIXME: when Opt-in Built-in Types becomes a thing, we can force these structs
//to be Send. For now, this has the compiler do a static check.
fn _assert_send<T: Send>() {
    _assert_send::<client::Request<net::Fresh>>();
    _assert_send::<client::Response>();

    _assert_send::<server::Request>();
    _assert_send::<server::Response<net::Fresh>>();
}
