#![deny(warnings)]
extern crate hyper;
extern crate futures;
extern crate tokio_core;
extern crate pretty_env_logger;

use futures::future::FutureResult;

use hyper::{Get, StatusCode};
use tokio_core::reactor::Core;
use hyper::header::ContentLength;
use hyper::server::{Http, Service, Request, Response};

static INDEX1: &'static [u8] = b"The 1st service!";
static INDEX2: &'static [u8] = b"The 2nd service!";

struct Service1;
struct Service2;

impl Service for Service1 {
    type Request = Request;
    type Response = Response;
    type Error = hyper::Error;
    type Future = FutureResult<Response, hyper::Error>;

    fn call(&self, req: Request) -> Self::Future {
        futures::future::ok(match (req.method(), req.path()) {
            (&Get, "/") => {
                Response::new()
                    .with_header(ContentLength(INDEX1.len() as u64))
                    .with_body(INDEX1)
            },
            _ => {
                Response::new()
                    .with_status(StatusCode::NotFound)
            }
        })
    }

}

impl Service for Service2 {
    type Request = Request;
    type Response = Response;
    type Error = hyper::Error;
    type Future = FutureResult<Response, hyper::Error>;

    fn call(&self, req: Request) -> Self::Future {
        futures::future::ok(match (req.method(), req.path()) {
            (&Get, "/") => {
                Response::new()
                    .with_header(ContentLength(INDEX2.len() as u64))
                    .with_body(INDEX2)
            },
            _ => {
                Response::new()
                    .with_status(StatusCode::NotFound)
            }
        })
    }

}


fn main() {
    pretty_env_logger::init().unwrap();
    let addr1 = "127.0.0.1:1337".parse().unwrap();
    let addr2 = "127.0.0.1:1338".parse().unwrap();

    let mut core = Core::new().unwrap();
    let handle = core.handle();

    let srv1 = Http::new().bind_handle(&addr1,|| Ok(Service1), &handle).unwrap();
    let srv2 = Http::new().bind_handle(&addr2,|| Ok(Service2), &handle).unwrap();

    println!("Listening on http://{}", srv1.local_addr().unwrap());
    println!("Listening on http://{}", srv2.local_addr().unwrap());

    handle.spawn(srv1.shutdown_signal(futures::future::empty::<(), ()>()));
    handle.spawn(srv2.shutdown_signal(futures::future::empty::<(), ()>()));
    core.run(futures::future::empty::<(), ()>()).unwrap();
}
