#![deny(warnings)]
extern crate hyper;
extern crate futures;
extern crate pretty_env_logger;
extern crate tokio;

use futures::{Future};
use futures::future::{FutureResult, lazy};

use hyper::{Body, Method, Request, Response, StatusCode};
use hyper::server::{Server, Service};

static INDEX1: &'static [u8] = b"The 1st service!";
static INDEX2: &'static [u8] = b"The 2nd service!";

struct Srv(&'static [u8]);

impl Service for Srv {
    type Request = Request<Body>;
    type Response = Response<Body>;
    type Error = hyper::Error;
    type Future = FutureResult<Response<Body>, hyper::Error>;

    fn call(&self, req: Request<Body>) -> Self::Future {
        futures::future::ok(match (req.method(), req.uri().path()) {
            (&Method::GET, "/") => {
                Response::new(self.0.into())
            },
            _ => {
                Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .body(Body::empty())
                    .unwrap()
            }
        })
    }

}


fn main() {
    pretty_env_logger::init();

    let addr1 = ([127, 0, 0, 1], 1337).into();
    let addr2 = ([127, 0, 0, 1], 1338).into();

    tokio::run(lazy(move || {
        let srv1 = Server::bind(&addr1)
            .serve(|| Ok(Srv(INDEX1)))
            .map_err(|e| eprintln!("server 1 error: {}", e));

        let srv2 = Server::bind(&addr2)
            .serve(|| Ok(Srv(INDEX2)))
            .map_err(|e| eprintln!("server 2 error: {}", e));

        println!("Listening on http://{} and http://{}", addr1, addr2);

        tokio::spawn(srv1);
        tokio::spawn(srv2);

        Ok(())
    }));
}
