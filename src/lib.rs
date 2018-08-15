#![doc(html_root_url = "https://docs.rs/hyper/0.11.27")]
#![deny(missing_docs)]
#![cfg_attr(test, deny(warnings))]
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

extern crate bytes;
#[macro_use] extern crate futures;
extern crate futures_cpupool;
#[cfg(feature = "compat")]
extern crate http;
extern crate httparse;
extern crate hyper_old_types;
extern crate iovec;
#[macro_use] extern crate log;
extern crate net2;
extern crate relay;
extern crate time;
extern crate tokio_core as tokio;
#[macro_use] extern crate tokio_io;
#[cfg(feature = "tokio-proto")]
extern crate tokio_proto;
extern crate tokio_service;
extern crate want;

#[cfg(all(test, feature = "nightly"))]
extern crate test;

pub use hyper_old_types::{
    error,
    header,
    mime,

    Error,
    Headers,
    HttpVersion,
    Method,
    Result,
    StatusCode,
    Uri,
};

pub use client::Client;
pub use proto::{Body, Chunk};
pub use proto::request::Request;
pub use proto::response::Response;
pub use Method::{Get, Head, Post, Put, Delete};
pub use StatusCode::{Ok, BadRequest, NotFound};
pub use server::Server;
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
mod macros;
mod method {
    pub use super::Method;
}
mod proto;
pub mod server;
mod status {
    pub use super::StatusCode;
}
mod uri {
    pub use super::Uri;
}
mod version {
    pub use super::HttpVersion;
}
