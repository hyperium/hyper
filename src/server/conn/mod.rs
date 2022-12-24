//!  Server connection API.
//!
//! The types in this module are to provide a lower-level API based around a
//! single connection. Accepting a connection and binding it with a service
//! are not handled at this level. This module provides the building blocks to
//! customize those things externally.
//!
//! This module is split by HTTP version. Both work similarly, but do have
//! specific options on each builder.
//!
//! ## Example
//!
//! A simple example that prepares an HTTP/1 connection over a Tokio TCP stream.
//!
//! ```no_run
//! # #[cfg(feature = "http1")]
//! # mod rt {
//! use http::{Request, Response, StatusCode};
//! use http_body_util::Full;
//! use hyper::{server::conn::http1, service::service_fn, body, body::Bytes};
//! use std::{net::SocketAddr, convert::Infallible};
//! use tokio::net::TcpListener;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
//!     let addr: SocketAddr = ([127, 0, 0, 1], 8080).into();
//!
//!     let mut tcp_listener = TcpListener::bind(addr).await?;
//!     loop {
//!         let (tcp_stream, _) = tcp_listener.accept().await?;
//!         tokio::task::spawn(async move {
//!             if let Err(http_err) = http1::Builder::new()
//!                     .keep_alive(true)
//!                     .serve_connection(tcp_stream, service_fn(hello))
//!                     .await {
//!                 eprintln!("Error while serving HTTP connection: {}", http_err);
//!             }
//!         });
//!     }
//! }
//!
//! async fn hello(_req: Request<body::Incoming>) -> Result<Response<Full<Bytes>>, Infallible> {
//!    Ok(Response::new(Full::new(Bytes::from("Hello World!"))))
//! }
//! # }
//! ```

#[cfg(feature = "http1")]
pub mod http1;
#[cfg(feature = "http2")]
pub mod http2;

