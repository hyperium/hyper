#![doc(html_root_url = "https://hyperium.github.io/hyper/")]
#![cfg_attr(test, deny(missing_docs))]
#![cfg_attr(test, deny(warnings))]
#![cfg_attr(all(test, feature = "nightly"), feature(test))]
#![cfg_attr(feature = "timeouts", feature(duration, socket_timeout))]

//! # Hyper
//!
//! Hyper is a fast, modern HTTP implementation written in and for Rust. It
//! is a low-level typesafe abstraction over raw HTTP, providing an elegant
//! layer over "stringly-typed" HTTP.
//!
//! Hyper offers both a [Client](client/index.html) and a
//! [Server](server/index.html) which can be used to drive complex web
//! applications written entirely in Rust.
//!
//! ## Internal Design
//!
//! Hyper is designed as a relatively low-level wrapper over raw HTTP. It should
//! allow the implementation of higher-level abstractions with as little pain as
//! possible, and should not irrevocably hide any information from its users.
//!
//! ### Common Functionality
//!
//! Functionality and code shared between the Server and Client implementations
//! can be found in `src` directly - this includes `NetworkStream`s, `Method`s,
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
//! Hyper's [header](header/index.html) representation is likely the most
//! complex API exposed by Hyper.
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
//! A `Handler` in Hyper accepts a `Request` and `Response`. This is where
//! user-code can handle each connection. The server accepts connections in a
//! task pool with a customizable number of threads, and passes the Request /
//! Response to the handler.
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
//! in the documentation for server Response.
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

extern crate rustc_serialize as serialize;
extern crate time;
extern crate url;
#[cfg(feature = "openssl")]
extern crate openssl;
#[cfg(feature = "serde-serialization")]
extern crate serde;
extern crate cookie;
extern crate unicase;
extern crate httparse;
extern crate num_cpus;
extern crate traitobject;
extern crate typeable;
extern crate solicit;

#[macro_use]
extern crate language_tags;

#[macro_use]
extern crate mime as mime_crate;

#[macro_use]
extern crate log;

#[cfg(all(test, feature = "nightly"))]
extern crate test;


pub use url::Url;
pub use client::Client;
pub use error::{Result, Error};
pub use method::Method::{Get, Head, Post, Delete};
pub use status::StatusCode::{Ok, BadRequest, NotFound};
pub use server::Server;
pub use language_tags::LanguageTag;

macro_rules! todo(
    ($($arg:tt)*) => (if cfg!(not(ndebug)) {
        trace!("TODO: {:?}", format_args!($($arg)*))
    })
);

macro_rules! inspect(
    ($name:expr, $value:expr) => ({
        let v = $value;
        trace!("inspect: {:?} = {:?}", $name, v);
        v
    })
);

#[cfg(test)]
#[macro_use]
mod mock;
#[doc(hidden)]
pub mod buffer;
pub mod client;
pub mod error;
pub mod method;
pub mod header;
pub mod http;
pub mod net;
pub mod server;
pub mod status;
pub mod uri;
pub mod version;

/// Re-exporting the mime crate, for convenience.
pub mod mime {
    pub use mime_crate::*;
}

#[allow(unconditional_recursion)]
fn _assert_send<T: Send>() {
    _assert_send::<Client>();
    _assert_send::<client::Request<net::Fresh>>();
    _assert_send::<client::Response>();
    _assert_send::<error::Error>();
}

#[allow(unconditional_recursion)]
fn _assert_sync<T: Sync>() {
    _assert_sync::<Client>();
    _assert_sync::<error::Error>();
}
