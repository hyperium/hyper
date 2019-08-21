#![doc(html_root_url = "https://docs.rs/hyper/0.12.32")]
#![deny(missing_docs)]
#![deny(missing_debug_implementations)]
// XXX NOOOOOOOO
//#![cfg_attr(test, deny(warnings))]
#![allow(warnings)]
#![cfg_attr(all(test, feature = "nightly"), feature(test))]

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

#[doc(hidden)] pub extern crate http;
#[macro_use] extern crate log;

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

pub use crate::client::Client;
pub use crate::error::{Result, Error};
pub use crate::body::{Body, Chunk};
pub use crate::server::Server;

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
