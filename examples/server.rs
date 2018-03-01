#![deny(warnings)]
extern crate futures;
extern crate hyper;
extern crate pretty_env_logger;

use futures::future::FutureResult;

use hyper::{Body, Method, Request, Response, StatusCode};
use hyper::server::{Http, Service};

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
                Response::new(req.into_parts().1)
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
    let addr = "127.0.0.1:1337".parse().unwrap();

    let server = Http::new().bind(&addr, || Ok(Echo)).unwrap();
    println!("Listening on http://{} with 1 thread.", server.local_addr().unwrap());
    server.run().unwrap();
}
