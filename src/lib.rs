#![feature(macro_rules, phase, default_type_params, slicing_syntax, globs)]
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

extern crate url;

pub use std::io::net::ip::{SocketAddr, IpAddr, Ipv4Addr, Ipv6Addr, Port};
pub use mimewrapper::mime;
pub use url::Url;

#[cfg(feature = "hyper-client")]
pub use import_wrapper::client::Client;
#[cfg(feature = "hyper-client")]
pub use import_wrapper::client;
#[cfg(feature = "hyper-server")]
pub use import_wrapper::server::Server;
#[cfg(feature = "hyper-server")]
pub use import_wrapper::server;
#[cfg(feature = "hyper-net")]
pub use import_wrapper::net;

pub use import_wrapper::protocol;

pub use import_wrapper::core;
pub use import_wrapper::core::method::Method::{Get, Head, Post, Delete};
pub use import_wrapper::core::status::StatusCode::{Ok, BadRequest, NotFound};
pub use import_wrapper::core::{HttpResult, HttpError};

#[cfg(test)]
#[macro_escape]
mod mock;

mod import_wrapper {

    extern crate "hyper-core" as core;
    extern crate "hyper-protocol" as protocol;

    #[cfg(feature = "hyper-net")]
    extern crate "hyper-net" as net;
    #[cfg(feature = "hyper-server")]
    extern crate "hyper-server" as server;
    #[cfg(feature = "hyper-client")]
    extern crate "hyper-client" as client;
}

mod mimewrapper {
    /// Re-exporting the mime crate, for convenience.
    extern crate mime;
}
