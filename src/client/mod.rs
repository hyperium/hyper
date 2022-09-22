//! HTTP Client
//!
//! hyper provides HTTP over a single connection. See the [`conn`](conn) module.
//!
//! ## Example
//!
//! For a small example program simply fetching a URL, take a look at the
//! [full client example](https://github.com/hyperium/hyper/blob/master/examples/client.rs).

#[cfg(test)]
mod tests;

cfg_feature! {
    #![any(feature = "http1", feature = "http2")]

    pub mod conn;
    pub(super) mod dispatch;
}
