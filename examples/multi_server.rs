#![deny(warnings)]
extern crate hyper;
extern crate futures;
extern crate tokio_core;
extern crate pretty_env_logger;

use futures::{Future, Stream};
use futures::future::FutureResult;

use hyper::{Get, StatusCode};
use tokio_core::reactor::Core;
use hyper::header::ContentLength;
use hyper::server::{Http, Service, Request, Response};

static INDEX1: &'static [u8] = b"The 1st service!";
static INDEX2: &'static [u8] = b"The 2nd service!";

struct Srv(&'static [u8]);

impl Service for Srv {
    type Request = Request;
    type Response = Response;
    type Error = hyper::Error;
    type Future = FutureResult<Response, hyper::Error>;

    fn call(&self, req: Request) -> Self::Future {
        futures::future::ok(match (req.method(), req.path()) {
            (&Get, "/") => {
                Response::new()
                    .with_header(ContentLength(self.0.len() as u64))
                    .with_body(self.0)
            },
            _ => {
                Response::new()
                    .with_status(StatusCode::NotFound)
            }
        })
    }

}


fn main() {
    pretty_env_logger::init();
    let addr1 = "127.0.0.1:1337".parse().unwrap();
    let addr2 = "127.0.0.1:1338".parse().unwrap();

    let mut core = Core::new().unwrap();
    let handle = core.handle();

    let srv1 = Http::new().serve_addr_handle(&addr1, &handle, || Ok(Srv(INDEX1))).unwrap();
    let srv2 = Http::new().serve_addr_handle(&addr2, &handle, || Ok(Srv(INDEX2))).unwrap();

    println!("Listening on http://{}", srv1.incoming_ref().local_addr());
    println!("Listening on http://{}", srv2.incoming_ref().local_addr());

    let handle1 = handle.clone();
    handle.spawn(srv1.for_each(move |conn| {
        handle1.spawn(conn.map(|_| ()).map_err(|err| println!("srv1 error: {:?}", err)));
        Ok(())
    }).map_err(|_| ()));

    let handle2 = handle.clone();
    handle.spawn(srv2.for_each(move |conn| {
        handle2.spawn(conn.map(|_| ()).map_err(|err| println!("srv2 error: {:?}", err)));
        Ok(())
    }).map_err(|_| ()));

    core.run(futures::future::empty::<(), ()>()).unwrap();
}
