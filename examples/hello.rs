#![deny(warnings)]
extern crate hyper;
extern crate futures;
extern crate pretty_env_logger;
extern crate tokio;

use futures::FutureExt;
use futures::future::lazy;

use hyper::{Body, Response};
use hyper::server::{Http, const_service, service_fn};

static PHRASE: &'static [u8] = b"Hello World!";

fn main() {
    pretty_env_logger::init();
    let addr = ([127, 0, 0, 1], 3000).into();

    let new_service = const_service(service_fn(|_| {
        Ok(Response::new(Body::from(PHRASE)))
    }));

    tokio::runtime::run2(lazy(move |_| {
        let server = Http::new()
            .sleep_on_errors(true)
            .bind(&addr, new_service)
            .unwrap();

        println!("Listening on http://{}", server.local_addr().unwrap());
        server.run().map_err(|err| panic!("Server error {}", err))
    }));
}
