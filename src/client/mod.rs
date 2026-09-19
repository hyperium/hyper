//! HTTP Client.
//!
//! hyper provides HTTP over a single connection. See the [`conn`] module.
//!
//! Whether a connection speaks HTTP/1 or HTTP/2 is decided when that
//! connection is established — for TLS, typically via ALPN — not by the
//! [`Version`] on each [`Request`]. See [`conn`] for details.
//!
//! ## Examples
//!
//! * [`client`] - A simple CLI http client that requests the url passed in parameters and outputs the response content and details to the stdout, reading content chunk-by-chunk.
//!
//! * [`client_json`] - A simple program that GETs some json, reads the body asynchronously, parses it with serde and outputs the result.
//!
//! [`client`]: https://github.com/hyperium/hyper/blob/master/examples/client.rs
//! [`client_json`]: https://github.com/hyperium/hyper/blob/master/examples/client_json.rs
//! [`Request`]: crate::Request
//! [`Version`]: crate::Version

#[cfg(test)]
mod tests;

cfg_feature! {
    #![any(feature = "http1", feature = "http2")]

    pub mod conn;
    pub(super) mod dispatch;
}
