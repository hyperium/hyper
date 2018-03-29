#![deny(warnings)]
extern crate futures;
extern crate hyper;
extern crate pretty_env_logger;
extern crate tokio;

use futures::FutureExt;
use futures::future::{FutureResult, lazy};

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
    let addr = "127.0.0.1:1337".parse().unwrap();

    tokio::runtime::run2(lazy(move |_| {
        let server = Http::new().bind(&addr, || Ok(Echo)).unwrap();
        println!("Listening on http://{}", server.local_addr().unwrap());
        server.run().recover(|err| {
            eprintln!("Server error {}", err)
        })
    }));
}
