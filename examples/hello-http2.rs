#![deny(warnings)]

use std::convert::Infallible;
use std::net::SocketAddr;

use hyper::body::Bytes;
use http_body_util::Full;
use hyper::server::conn::http2;
use hyper::service::service_fn;
use hyper::{Request, Response};
use tokio::net::TcpListener;

// This would normally come from the `hyper-util` crate, but we can't depend
// on that here because it would be a cyclical dependency.
#[path = "../benches/support/mod.rs"]
mod support;
use support::{TokioIo, TokioTimer};

// An async function that consumes a request, does nothing with it and returns a
// response.
async fn hello(_: Request<hyper::body::Incoming>) -> Result<Response<Full<Bytes>>, Infallible> {
    Ok(Response::new(Full::new(Bytes::from("Hello, World!"))))
}

#[derive(Clone)]
// An Executor that uses the tokio runtime.
pub struct TokioExecutor;

// Implement the `hyper::rt::Executor` trait for `TokioExecutor` so that it can be used to spawn
// tasks in the hyper runtime. 
// An Executor allows us to manage execution of tasks which can help us improve the efficiency and
// scalability of the server.
impl<F> hyper::rt::Executor<F> for TokioExecutor
where
    F: std::future::Future + Send + 'static,
    F::Output: Send + 'static,
    {
        fn execute(&self, fut: F) {
        tokio::task::spawn(fut);    
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    pretty_env_logger::init();

    // This address is localhost
    let addr = SocketAddr::from(([127,0,0,1], 3000));

    // Bind to the port and listen for incoming TCP connections
    let listener = TcpListener::bind(addr).await?;

    loop {
        // When an incoming TCP connection is received grab a TCP stream for
        // client<->server communication.
        //
        // Note, this is a .await point, this loop will loop forever but is not a busy loop. The
        // .await point allows the Tokio runtime to pull the task off of the thread until the task
        // has work to do. In this case, a connection arrives on the port we are listening on and
        // the task is woken up, at which point the task is then put back on a thread, and is
        // driven forward by the runtime, eventually yielding a TCP stream.
        let (stream, _) = listener.accept().await?;
        // Use an adapter to access something implementing `tokio::io` traits as if they implement
        // `hyper::rt` IO traits.
        let io = TokioIo::new(stream);

        // Spin up a new task in Tokio so we can continue to listen for new TCP connection on the
        // current task without waiting for the processing of the HTTP2 connection we just received
        // to finish
        tokio::task::spawn(async move{
            // Handle the connection from the client using HTTP2 with an executor and pass any
            // HTTP requests received on that connection to the `hello` function
            if let Err(err) = http2::Builder::new(TokioExecutor)
                .serve_connection(io, service_fn(hello))
                .await
            {
                eprintln!("Error serving connection: {}", err);
            }
        });
    }
}
