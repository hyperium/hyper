#![doc(html_root_url = "https://docs.rs/hyper/0.11.12")]
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
//! If just starting out, **check out the [Guides](https://hyper.rs/guides)
//! first.**

extern crate base64;
extern crate bytes;
#[macro_use] extern crate futures;
extern crate futures_cpupool;
#[cfg(feature = "compat")]
extern crate http;
extern crate httparse;
extern crate language_tags;
#[macro_use] extern crate log;
pub extern crate mime;
#[macro_use] extern crate percent_encoding;
extern crate relay;
extern crate time;
extern crate tokio_core as tokio;
#[macro_use] extern crate tokio_io;
#[cfg(feature = "tokio-proto")]
extern crate tokio_proto;
extern crate tokio_service;
extern crate unicase;

#[cfg(all(test, feature = "nightly"))]
extern crate test;

pub use uri::Uri;
pub use client::Client;
pub use error::{Result, Error};
pub use header::Headers;
pub use proto::{Body, Chunk};
pub use proto::request::Request;
pub use proto::response::Response;
pub use method::Method::{self, Get, Head, Post, Put, Delete};
pub use status::StatusCode::{self, Ok, BadRequest, NotFound};
pub use server::Server;
pub use version::HttpVersion;
#[cfg(feature = "raw_status")]
pub use proto::RawStatus;

macro_rules! feat_server_proto {
    ($($i:item)*) => ($(
        #[cfg(feature = "server-proto")]
        #[deprecated(
            since="0.11.11",
            note="All usage of the tokio-proto crate is going away."
        )]
        #[doc(hidden)]
        #[allow(deprecated)]
        $i
    )*)
}

mod common;
#[cfg(test)]
mod mock;
pub mod client;
pub mod error;
mod method;
pub mod header;
mod proto;
pub mod server;
mod status;
mod uri;
mod version;
