#![doc(html_root_url = "https://docs.rs/hyper/0.11.22")]
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
//! [Server](server/index.html).
//!
//! If just starting out, **check out the [Guides](https://hyper.rs/guides)
//! first.**

extern crate bytes;
#[macro_use] extern crate futures;
extern crate futures_cpupool;
extern crate futures_timer;
extern crate h2;
extern crate http;
extern crate httparse;
extern crate iovec;
#[macro_use] extern crate log;
extern crate net2;
extern crate time;
extern crate tokio;
extern crate tokio_executor;
#[macro_use] extern crate tokio_io;
extern crate tokio_service;
extern crate want;

#[cfg(all(test, feature = "nightly"))]
extern crate test;

pub use http::{
    HeaderMap,
    Method,
    Request,
    Response,
    StatusCode,
    Uri,
    Version,
};

pub use client::Client;
pub use error::{Result, Error};
pub use body::{Body};
pub use chunk::Chunk;
pub use server::Server;

mod common;
#[cfg(test)]
mod mock;
pub mod body;
mod chunk;
pub mod client;
pub mod error;
mod headers;
mod proto;
pub mod server;
