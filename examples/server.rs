#![deny(warnings)]
extern crate futures;
extern crate hyper;
extern crate pretty_env_logger;
extern crate tokio;

use futures::Future;
use futures::future::{FutureResult};

use hyper::{Body, Method, Request, Response, StatusCode};
use hyper::server::{Server, Service};

static INDEX: &'static [u8] = b"Try POST /echo";

struct Echo;

impl Service for Echo {
    type Request = Request<Body>;
    type Response = Response<Body>;
    type Error = hyper::Error;
    type Future = FutureResult<Self::Response, Self::Error>;

    fn call(&self, req: Self::Request) -> Self::Future {
        futures::future::ok(match (req.method(), req.uri().path()) {
            (&Method::GET, "/") | (&Method::POST, "/") => {
                Response::new(INDEX.into())
            },
            (&Method::POST, "/echo") => {
                Response::new(req.into_body())
            },
            _ => {
                let mut res = Response::new(Body::empty());
                *res.status_mut() = StatusCode::NOT_FOUND;
                res
            }
        })
    }

}


fn main() {
    pretty_env_logger::init();

    let addr = ([127, 0, 0, 1], 1337).into();

    let server = Server::bind(&addr)
        .serve(|| Ok(Echo))
        .map_err(|e| eprintln!("server error: {}", e));

    println!("Listening on http://{}", addr);

    tokio::run(server);
}
