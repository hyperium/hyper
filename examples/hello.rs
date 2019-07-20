#![feature(async_await)]
#![deny(warnings)]

use hyper::{Body, Request, Response, Server};
use hyper::service::{make_service_fn, service_fn};

async fn hello(_: Request<Body>) -> Result<Response<Body>, hyper::Error> {
    Ok(Response::new(Body::from("Hello World!")))
}

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    pretty_env_logger::init();

    let addr = ([127, 0, 0, 1], 3000).into();

    let server = Server::bind(&addr)
        .serve(make_service_fn(|_| {
            // This is the `Service` that will handle the connection.
            // `service_fn` is a helper to convert a function that
            // returns a Response into a `Service`.
            async {
                Ok::<_, hyper::Error>(service_fn(hello))
            }
        }));

    println!("Listening on http://{}", addr);

    server.await?;

    Ok(())
}
