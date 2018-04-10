#![deny(warnings)]
extern crate hyper;
extern crate futures;
extern crate pretty_env_logger;
extern crate tokio;

use futures::Future;
use futures::future::lazy;

use hyper::{Body, Response};
use hyper::server::{Http, const_service, service_fn};

static PHRASE: &'static [u8] = b"Hello World!";

fn main() {
    pretty_env_logger::init();
    let addr = ([127, 0, 0, 1], 3000).into();

    let new_service = const_service(service_fn(|_| {
        //TODO: when `!` is stable, replace error type
        Ok::<_, hyper::Error>(Response::new(Body::from(PHRASE)))
    }));

    tokio::run(lazy(move || {
        let server = Http::new()
            .sleep_on_errors(true)
            .bind(&addr, new_service)
            .unwrap();

        println!("Listening on http://{} with 1 thread.", server.local_addr().unwrap());
        server.run().map_err(|err| eprintln!("Server error {}", err))
    }));
}
