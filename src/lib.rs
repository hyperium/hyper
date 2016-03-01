#![doc(html_root_url = "https://hyperium.github.io/hyper/")]
//#![cfg_attr(test, deny(missing_docs))]
#![deny(missing_docs)]
#![deny(warnings)]
#![deny(missing_debug_implementations)]
#![cfg_attr(all(test, feature = "nightly"), feature(test))]

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
extern crate mio;
extern crate rotor;
extern crate traitobject;
extern crate typeable;
extern crate vecio;

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
pub use http::{Next, Encoder, Decoder, Control};
pub use header::Headers;
pub use method::Method::{self, Get, Head, Post, Delete};
pub use status::StatusCode::{self, Ok, BadRequest, NotFound};
pub use server::Server;
pub use uri::RequestUri;
pub use version::HttpVersion;
pub use language_tags::LanguageTag;

macro_rules! unimplemented {
    () => (unimplemented!(""));
    ($($arg:tt)*) => ({
        panic!("unimplemented: {}", format_args!($($arg)*))
    })
}

macro_rules! rotor_try {
    ($e:expr) => ({
        match $e {
            Ok(v) => v,
            Err(e) => return ::rotor::Response::error(e.into())
        }
    });
}

macro_rules! todo(
    ($($arg:tt)*) => (if cfg!(not(ndebug)) {
        trace!("TODO: {:?}", format_args!($($arg)*))
    })
);

macro_rules! inspect(
    ($value:expr) => ({
        inspect!(stringify!($value), $value)
    });
    ($name:expr, $value:expr) => ({
        let v = $value;
        trace!("inspect: {:?} = {:?}", $name, v);
        v
    })
);

#[cfg(test)]
mod mock;
pub mod client;
pub mod error;
pub mod method;
pub mod header;
mod http;
pub mod net;
pub mod server;
pub mod status;
pub mod uri;
pub mod version;

/// Re-exporting the mime crate, for convenience.
pub mod mime {
    pub use mime_crate::*;
}

/*
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
*/
