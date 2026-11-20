#![deny(warnings)]

use std::convert::Infallible;
use std::net::SocketAddr;
use std::time::Duration;

use bytes::Bytes;
use http_body_util::Full;
use hyper::server::conn::http1;
use hyper::{body::Incoming, Request, Response};
use tokio::net::TcpListener;
use tower::{ServiceBuilder, ServiceExt};

// This would normally come from the `hyper-util` crate, but we can't depend
// on that here because it would be a cyclical dependency.
#[path = "../benches/support/mod.rs"]
mod support;
use support::{TokioIo, TokioTimer};

// The same handler as in `hello.rs`. `tower::service_fn` turns it into a
// `tower::Service`, which is what a `tower::Layer` knows how to wrap.
async fn hello(_: Request<Incoming>) -> Result<Response<Full<Bytes>>, Infallible> {
    Ok(Response::new(Full::new(Bytes::from("Hello World!"))))
}

// `tower::Service` and `hyper::service::Service` are different traits, so a
// tower stack cannot be given to `serve_connection` as it is. They differ in
// two ways:
//
// - tower's `call` takes `&mut self` while hyper's takes `&self`, so the tower
//   service has to be cloned for each request.
// - a tower service must be driven to readiness with `poll_ready` before every
//   `call`, and hyper has no `poll_ready` to drive it from. Layers such as
//   `concurrency_limit` acquire their resources there, so skipping it would
//   break them.
//
// `ServiceExt::oneshot` does both: it takes a service by value, polls it until
// it is ready, and then calls it.
//
// `hyper_util::service::TowerToHyperService` is this same adapter and is what
// to use in a real application. It cannot be used here because hyper cannot
// depend on hyper-util.
#[derive(Clone)]
struct TowerToHyperService<S> {
    service: S,
}

impl<S, R> hyper::service::Service<R> for TowerToHyperService<S>
where
    S: tower::Service<R> + Clone,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = tower::util::Oneshot<S, R>;

    fn call(&self, req: R) -> Self::Future {
        self.service.clone().oneshot(req)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    pretty_env_logger::init();

    // This address is localhost
    let addr: SocketAddr = ([127, 0, 0, 1], 3000).into();

    // Layers wrap outside-in: a request passes the concurrency limit, then the
    // timeout, and only then reaches `hello`.
    //
    // The stack is built here and not inside the accept loop, so that the
    // semaphore behind `concurrency_limit` is shared by every connection.
    // Building it per connection would limit each connection on its own.
    //
    // The concurrency limit takes its permit in `poll_ready`, which the adapter
    // above awaits before `timeout` starts its timer in `call`. The timeout
    // therefore bounds the handler and not the time spent waiting for a permit,
    // whichever order the two layers are written in.
    let service = ServiceBuilder::new()
        .concurrency_limit(128)
        .timeout(Duration::from_secs(10))
        .service(tower::service_fn(hello));
    let service = TowerToHyperService { service };

    let listener = TcpListener::bind(addr).await?;
    println!("Listening on http://{}", addr);

    loop {
        let (tcp, _) = listener.accept().await?;
        let io = TokioIo::new(tcp);

        // Cloning the stack is cheap and keeps the shared semaphore.
        let service = service.clone();

        tokio::task::spawn(async move {
            // An error from the service, an elapsed timeout for instance,
            // aborts the connection. A server that would rather answer with a
            // 503 or a 504 needs one more layer turning the error into a
            // response.
            if let Err(err) = http1::Builder::new()
                .timer(TokioTimer::new())
                .serve_connection(io, service)
                .await
            {
                println!("Error serving connection: {:?}", err);
            }
        });
    }
}
