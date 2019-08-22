#![deny(warnings)]
#![warn(rust_2018_idioms)]

use hyper::{Body, Request, Response, Server};
use hyper::service::{service_fn, make_service_fn};
use futures_util::future::join;

static INDEX1: &'static [u8] = b"The 1st service!";
static INDEX2: &'static [u8] = b"The 2nd service!";

async fn index1(_: Request<Body>) -> Result<Response<Body>, hyper::Error> {
    Ok(Response::new(Body::from(INDEX1)))
}

async fn index2(_: Request<Body>) -> Result<Response<Body>, hyper::Error> {
    Ok(Response::new(Body::from(INDEX2)))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    pretty_env_logger::init();

    let addr1 = ([127, 0, 0, 1], 1337).into();
    let addr2 = ([127, 0, 0, 1], 1338).into();

    let srv1 = Server::bind(&addr1)
        .serve(make_service_fn(|_| {
            async {
                Ok::<_, hyper::Error>(service_fn(index1))
            }
        }));

    let srv2 = Server::bind(&addr2)
        .serve(make_service_fn(|_| {
            async {
                Ok::<_, hyper::Error>(service_fn(index2))
            }
        }));

    println!("Listening on http://{} and http://{}", addr1, addr2);

    let _ret = join(srv1, srv2).await;

    Ok(())
}
