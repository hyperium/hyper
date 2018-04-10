#![deny(warnings)]
extern crate hyper;
extern crate futures;
extern crate pretty_env_logger;
extern crate tokio;

use futures::{Future, Stream};
use futures::future::{FutureResult, lazy};

use hyper::{Body, Method, Request, Response, StatusCode};
use hyper::server::{Http, Service};

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
    let addr1 = "127.0.0.1:1337".parse().unwrap();
    let addr2 = "127.0.0.1:1338".parse().unwrap();

    tokio::run(lazy(move || {
        let srv1 = Http::new().serve_addr(&addr1, || Ok(Srv(INDEX1))).unwrap();
        let srv2 = Http::new().serve_addr(&addr2, || Ok(Srv(INDEX2))).unwrap();

        println!("Listening on http://{}", srv1.incoming_ref().local_addr());
        println!("Listening on http://{}", srv2.incoming_ref().local_addr());

        tokio::spawn(srv1.for_each(move |conn| {
            tokio::spawn(conn.map(|_| ()).map_err(|err| println!("srv1 error: {:?}", err)));
            Ok(())
        }).map_err(|_| ()));

        tokio::spawn(srv2.for_each(move |conn| {
            tokio::spawn(conn.map(|_| ()).map_err(|err| println!("srv2 error: {:?}", err)));
            Ok(())
        }).map_err(|_| ()));

        Ok(())
    }));
}
