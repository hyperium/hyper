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
//! A simple example that uses the `SendRequest` struct to talk HTTP over a Tokio TCP stream
//! ```no_run
//! # #[cfg(all(feature = "client", feature = "http1", feature = "runtime"))]
//! # mod rt {
//! use tower::ServiceExt;
//! use http::{Request, StatusCode};
//! use hyper::{client::conn, Body};
//! use tokio::net::TcpStream;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let target_stream = TcpStream::connect("example.com:80").await?;
//!
//!     let (mut request_sender, connection) = conn::http1::handshake(target_stream).await?;
//!
//!     // spawn a task to poll the connection and drive the HTTP state
//!     tokio::spawn(async move {
//!         if let Err(e) = connection.await {
//!             eprintln!("Error in connection: {}", e);
//!         }
//!     });
//!
//!     let request = Request::builder()
//!         // We need to manually add the host header because SendRequest does not
//!         .header("Host", "example.com")
//!         .method("GET")
//!         .body(Body::from(""))?;
//!     let response = request_sender.send_request(request).await?;
//!     assert!(response.status() == StatusCode::OK);
//!
//!     // To send via the same connection again, it may not work as it may not be ready,
//!     // so we have to wait until the request_sender becomes ready.
//!     // TODO:(mike) how to handle readiness
//!     // request_sender.ready().await?;
//!     let request = Request::builder()
//!         .header("Host", "example.com")
//!         .method("GET")
//!         .body(Body::from(""))?;
//!     let response = request_sender.send_request(request).await?;
//!     assert!(response.status() == StatusCode::OK);
//!     Ok(())
//! }
//!
//! # }
//! ```

#[cfg(feature = "http1")]
pub mod http1;
#[cfg(feature = "http2")]
pub mod http2;


