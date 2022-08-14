//! HTTP Client
//!
//! There are two levels of APIs provided for construct HTTP clients:
//!
//! - The higher-level [`Client`](Client) type.
//! - The lower-level [`conn`](conn) module.
//!
//! # Client
//!
//! The [`Client`](Client) is the main way to send HTTP requests to a server.
//! The default `Client` provides these things on top of the lower-level API:
//!
//! - A default **connector**, able to resolve hostnames and connect to
//!   destinations over plain-text TCP.
//! - A **pool** of existing connections, allowing better performance when
//!   making multiple requests to the same hostname.
//! - Automatic setting of the `Host` header, based on the request `Uri`.
//! - Automatic request **retries** when a pooled connection is closed by the
//!   server before any bytes have been written.
//!
//! Many of these features can configured, by making use of
//! [`Client::builder`](Client::builder).
//!
//! ## Example
//!
//! For a small example program simply fetching a URL, take a look at the
//! [full client example](https://github.com/hyperium/hyper/blob/master/examples/client.rs).
//!

pub mod connect;
#[cfg(all(test, feature = "runtime"))]
mod tests;

cfg_feature! {
    #![any(feature = "http1", feature = "http2")]

    pub use self::client::{Builder, Client, ResponseFuture};

    mod client;
    pub mod conn;
    pub(super) mod dispatch;
    mod pool;
}
