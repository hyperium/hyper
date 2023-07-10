#![deny(warnings)]

use std::net::SocketAddr;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use bytes::Bytes;
use http_body_util::Full;
use hyper::{server::conn::http1, service::service_fn};
use hyper::{Error, Response};
use tokio::net::TcpListener;

#[path = "../benches/support/mod.rs"]
mod support;
use support::TokioIo;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    pretty_env_logger::init();

    let addr: SocketAddr = ([127, 0, 0, 1], 3000).into();

    // For the most basic of state, we just share a counter, that increments
    // with each request, and we send its value back in the response.
    let counter = Arc::new(AtomicUsize::new(0));

    let listener = TcpListener::bind(addr).await?;
    println!("Listening on http://{}", addr);
    loop {
        let (stream, _) = listener.accept().await?;
        let io = TokioIo::new(stream);

        // Each connection could send multiple requests, so
        // the `Service` needs a clone to handle later requests.
        let counter = counter.clone();

        // This is the `Service` that will handle the connection.
        // `service_fn` is a helper to convert a function that
        // returns a Response into a `Service`.
        let service = service_fn(move |_req| {
            // Get the current count, and also increment by 1, in a single
            // atomic operation.
            let count = counter.fetch_add(1, Ordering::AcqRel);
            async move {
                Ok::<_, Error>(Response::new(Full::new(Bytes::from(format!(
                    "Request #{}",
                    count
                )))))
            }
        });

        if let Err(err) = http1::Builder::new().serve_connection(io, service).await {
            println!("Error serving connection: {:?}", err);
        }
    }
}
