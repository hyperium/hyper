#![deny(warnings)]
extern crate hyper;
extern crate futures;
extern crate pretty_env_logger;
extern crate tokio;

use futures::{Future};
use futures::future::{lazy};

use hyper::{Body, Response, Server};
use hyper::service::service_fn_ok;

static INDEX1: &'static [u8] = b"The 1st service!";
static INDEX2: &'static [u8] = b"The 2nd service!";

fn main() {
    pretty_env_logger::init();

    let addr1 = ([127, 0, 0, 1], 1337).into();
    let addr2 = ([127, 0, 0, 1], 1338).into();

    tokio::run(lazy(move || {
        let srv1 = Server::bind(&addr1)
            .serve(|| service_fn_ok(|_| Response::new(Body::from(INDEX1))))
            .map_err(|e| eprintln!("server 1 error: {}", e));

        let srv2 = Server::bind(&addr2)
            .serve(|| service_fn_ok(|_| Response::new(Body::from(INDEX2))))
            .map_err(|e| eprintln!("server 2 error: {}", e));

        println!("Listening on http://{} and http://{}", addr1, addr2);

        tokio::spawn(srv1);
        tokio::spawn(srv2);

        Ok(())
    }));
}
