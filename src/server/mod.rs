//! HTTP Server
//!
//! A `Server` is created to listen on a port, parse HTTP requests, and hand
//! them off to a `Service`.
//!
//! There are two levels of APIs provide for constructing HTTP servers:
//!
//! - The higher-level [`Server`](Server) type.
//! - The lower-level [`conn`](conn) module.
//!
//! # Server
//!
//! The [`Server`](Server) is main way to start listening for HTTP requests.
//! It wraps a listener with a [`MakeService`](crate::service), and then should
//! be executed to start serving requests.
//!
//! [`Server`](Server) accepts connections in both HTTP1 and HTTP2 by default.
pub mod accept;
pub mod conn;

pub use self::server::Server;

cfg_feature! {
    #![any(feature = "http1", feature = "http2")]

    pub(crate) mod server;
    pub use self::server::Builder;

    mod shutdown;
}

cfg_feature! {
    #![not(any(feature = "http1", feature = "http2"))]

    mod server_stub;
    use server_stub as server;
}
