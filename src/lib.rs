#![doc(html_root_url = "https://docs.rs/hyper/0.12.36")]
#![deny(missing_docs)]
#![deny(missing_debug_implementations)]
#![cfg_attr(test, deny(warnings))]
#![cfg_attr(all(test, feature = "nightly"), feature(test))]
#![allow(deprecated)]

//! # hyper
//!
//! hyper is a **fast** and **correct** HTTP implementation written in and for Rust.
//!
//! hyper provides both a [Client](client/index.html) and a
//! [Server](server/index.html).
//!
//! If just starting out, **check out the [Guides](https://hyper.rs/guides)
//! first.**
//!
//! If looking for just a convenient HTTP client, consider the
//! [reqwest](https://crates.io/crates/reqwest) crate.

extern crate bytes;
#[macro_use] extern crate futures;
#[cfg(feature = "runtime")] extern crate futures_cpupool;
extern crate h2;
#[doc(hidden)] pub extern crate http;
extern crate http_body;
extern crate httparse;
extern crate iovec;
extern crate itoa;
#[macro_use] extern crate log;
#[cfg(feature = "runtime")] extern crate net2;
extern crate time;
#[cfg(feature = "runtime")] extern crate tokio;
extern crate tokio_buf;
#[cfg(feature = "runtime")] extern crate tokio_executor;
#[macro_use] extern crate tokio_io;
#[cfg(feature = "runtime")] extern crate tokio_reactor;
#[cfg(feature = "runtime")] extern crate tokio_tcp;
#[cfg(feature = "runtime")] extern crate tokio_threadpool;
#[cfg(feature = "runtime")] extern crate tokio_timer;
extern crate want;

#[cfg(all(test, feature = "nightly"))]
extern crate test;

pub use http::{
    header,
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
pub use body::{Body, Chunk};
pub use server::Server;

#[macro_use]
mod common;
#[cfg(test)]
mod mock;
pub mod body;
pub mod client;
pub mod error;
mod headers;
mod proto;
pub mod server;
pub mod service;
#[cfg(feature = "runtime")] pub mod rt;
pub mod upgrade;
