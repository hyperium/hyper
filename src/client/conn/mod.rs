//! Lower-level client connection API.
//!
//! The types in this module are to provide a lower-level API based around a
//! single connection. Connecting to a host, pooling connections, and the like
//! are not handled at this level. This module provides the building blocks to
//! customize those things externally.
//!
//! If don't have need to manage connections yourself, consider using the
//! higher-level [Client](super) API.
//!
//! ## Example
//!
//! A simple example that uses the `SendRequest` struct to talk HTTP over some TCP stream.
//!
//! ```no_run
//! # #[cfg(all(feature = "client", feature = "http1"))]
//! # mod rt {
//! use bytes::Bytes;
//! use http::{Request, StatusCode};
//! use http_body_util::Empty;
//! use hyper::client::conn;
//! # use hyper::rt::{Read, Write};
//! # async fn run<I>(tcp: I) -> Result<(), Box<dyn std::error::Error>>
//! # where
//! #     I: Read + Write + Unpin + Send + 'static,
//! # {
//! let (mut request_sender, connection) = conn::http1::handshake(tcp).await?;
//!
//! // spawn a task to poll the connection and drive the HTTP state
//! tokio::spawn(async move {
//!     if let Err(e) = connection.await {
//!         eprintln!("Error in connection: {}", e);
//!     }
//! });
//!
//! let request = Request::builder()
//!     // We need to manually add the host header because SendRequest does not
//!     .header("Host", "example.com")
//!     .method("GET")
//!     .body(Empty::<Bytes>::new())?;
//!
//! let response = request_sender.send_request(request).await?;
//! assert!(response.status() == StatusCode::OK);
//!
//! let request = Request::builder()
//!     .header("Host", "example.com")
//!     .method("GET")
//!     .body(Empty::<Bytes>::new())?;
//!
//! let response = request_sender.send_request(request).await?;
//! assert!(response.status() == StatusCode::OK);
//! # Ok(())
//! # }
//! # }
//! ```

#[cfg(feature = "http1")]
pub mod http1;
#[cfg(feature = "http2")]
pub mod http2;
