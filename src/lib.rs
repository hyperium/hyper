#![doc(html_root_url = "https://hyperium.github.io/hyper/")]
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
//! Hyper provides both a [Client](client/index.html) and a
//! [Server](server/index.html), along with a
//! [typed Headers system](header/index.html).
//!
//! If just getting started, consider looking over the [Server Guide](./guide/server.html).
extern crate rustc_serialize as serialize;
extern crate time;
#[macro_use] extern crate url;
#[cfg(feature = "openssl")]
extern crate openssl;
#[cfg(feature = "openssl-verify")]
extern crate openssl_verify;
#[cfg(feature = "security-framework")]
extern crate security_framework;
#[cfg(feature = "serde-serialization")]
extern crate serde;
extern crate cookie;
extern crate unicase;
extern crate httparse;
extern crate rotor;
extern crate spmc;
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
pub use header::Headers;
pub use http::{Next, Encoder, Decoder, Control, ControlError};
pub use method::Method::{self, Get, Head, Post, Delete};
pub use net::{HttpStream, Transport};
pub use status::StatusCode::{self, Ok, BadRequest, NotFound};
pub use server::Server;
pub use uri::RequestUri;
pub use version::HttpVersion;

macro_rules! rotor_try {
    ($e:expr) => ({
        match $e {
            Ok(v) => v,
            Err(e) => return ::rotor::Response::error(e.into())
        }
    });
}

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
