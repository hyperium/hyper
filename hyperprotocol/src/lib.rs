#![feature(core, box_syntax, unsafe_destructor, into_cow, convert)]
#![cfg_attr(test, feature(test))]
#![deny(missing_docs)]
#![cfg_attr(test, deny(warnings))]

//! # Hyperprotocol
//! Hyperprotol contains the high-level semantics of HTTP. The provided code is used by both client
//! and server.

extern crate cookie;
extern crate httparse;
#[macro_use]
extern crate log;
extern crate mime as mime_crate;
extern crate rustc_serialize as serialize;
extern crate time;
extern crate unicase;
extern crate url;

#[cfg(test)]
extern crate test;

pub use mime_crate as mime;

pub mod error;
pub mod header;
pub mod method;
pub mod status;
pub mod version;
