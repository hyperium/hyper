#![deny(warnings)]

//! A Server-Sent Events (SSE) example.
//!
//! This server sends a stream of timestamped events to connected clients
//! using the `text/event-stream` content type.
//!
//! ```not_rust
//! cargo run --features="full" --example server_sent_events
//! ```
//!
//! Then connect with:
//!
//! ```not_rust
//! curl -N http://127.0.0.1:3000/
//! ```

use std::convert::Infallible;
use std::net::SocketAddr;
use std::time::Duration;

use bytes::Bytes;
use http_body_util::{combinators::BoxBody, BodyExt, StreamBody};
use hyper::body::Frame;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response};
use tokio::net::TcpListener;

#[path = "../benches/support/mod.rs"]
mod support;
use support::TokioIo;

async fn sse(
    _: Request<hyper::body::Incoming>,
) -> Result<Response<BoxBody<Bytes, Infallible>>, Infallible> {
    // Build an infinite stream that yields one SSE frame per second.
    let stream = futures_util::stream::unfold(0u64, |counter| async move {
        tokio::time::sleep(Duration::from_secs(1)).await;
        let counter = counter + 1;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // SSE format: each field ends with \n, events separated by \n\n
        let event = format!(
            "id: {counter}\nevent: tick\ndata: {{\"counter\":{counter},\"timestamp\":{now}}}\n\n"
        );
        let frame: Result<Frame<Bytes>, Infallible> = Ok(Frame::data(Bytes::from(event)));
        Some((frame, counter))
    });

    let body = StreamBody::new(stream).boxed();

    let response = Response::builder()
        .header("content-type", "text/event-stream")
        .header("cache-control", "no-cache")
        .body(body)
        .unwrap();

    Ok(response)
}

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    pretty_env_logger::init();

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    let listener = TcpListener::bind(addr).await?;
    println!("Listening on http://{addr}");

    loop {
        let (stream, _) = listener.accept().await?;
        let io = TokioIo::new(stream);

        tokio::task::spawn(async move {
            if let Err(err) = http1::Builder::new()
                .serve_connection(io, service_fn(sse))
                .await
            {
                eprintln!("Error serving connection: {err:?}");
            }
        });
    }
}
