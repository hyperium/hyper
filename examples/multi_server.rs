#![deny(warnings)]
extern crate hyper;
extern crate pretty_env_logger;

use hyper::{Body, Response, Server};
use hyper::service::service_fn_ok;
use hyper::rt::{self, Future};

static INDEX1: &'static [u8] = b"The 1st service!";
static INDEX2: &'static [u8] = b"The 2nd service!";

fn main() {
    pretty_env_logger::init();

    let addr1 = ([127, 0, 0, 1], 1337).into();
    let addr2 = ([127, 0, 0, 1], 1338).into();

    rt::run(rt::lazy(move || {
        let srv1 = Server::bind(&addr1)
            .serve(|| service_fn_ok(|_| Response::new(Body::from(INDEX1))))
            .map_err(|e| eprintln!("server 1 error: {}", e));

        let srv2 = Server::bind(&addr2)
            .serve(|| service_fn_ok(|_| Response::new(Body::from(INDEX2))))
            .map_err(|e| eprintln!("server 2 error: {}", e));

        println!("Listening on http://{} and http://{}", addr1, addr2);

        rt::spawn(srv1);
        rt::spawn(srv2);

        Ok(())
    }));
}
