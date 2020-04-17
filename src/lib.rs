#![doc(html_root_url = "https://docs.rs/hyper/0.13.5")]
#![deny(missing_docs)]
#![deny(missing_debug_implementations)]
#![cfg_attr(test, deny(rust_2018_idioms))]
#![cfg_attr(test, deny(warnings))]
#![cfg_attr(all(test, feature = "nightly"), feature(test))]

//! # hyper
//!
//! hyper is a **fast** and **correct** HTTP implementation written in and for Rust.
//!
//! ## Features
//!
//! - HTTP/1 and HTTP/2
//! - Asynchronous design
//! - Leading in performance
//! - Tested and **correct**
//! - Extensive production use
//! - [Client](client/index.html) and [Server](server/index.html) APIs
//!
//! If just starting out, **check out the [Guides](https://hyper.rs/guides)
//! first.**
//!
//! ## "Low-level"
//!
//! hyper is a lower-level HTTP library, meant to be a building block
//! for libraries and applications.
//!
//! If looking for just a convenient HTTP client, consider the
//! [reqwest](https://crates.io/crates/reqwest) crate.
//!
//! # Optional Features
//!
//! The following optional features are available:
//!
//! - `runtime` (*enabled by default*): Enables convenient integration with
//!   `tokio`, providing connectors and acceptors for TCP, and a default
//!   executor.
//! - `tcp` (*enabled by default*): Enables convenient implementations over
//!   TCP (using tokio).
//! - `stream` (*enabled by default*): Provides `futures::Stream` capabilities.

#[doc(hidden)]
pub use http;
#[macro_use]
extern crate log;

#[cfg(all(test, feature = "nightly"))]
extern crate test;

pub use http::{header, HeaderMap, Method, Request, Response, StatusCode, Uri, Version};

pub use crate::body::Body;
pub use crate::client::Client;
pub use crate::error::{Error, Result};
pub use crate::server::Server;

#[macro_use]
mod common;
pub mod body;
pub mod client;
#[doc(hidden)] // Mistakenly public...
pub mod error;
mod headers;
#[cfg(test)]
mod mock;
mod proto;
pub mod rt;
pub mod server;
pub mod service;
pub mod upgrade;
