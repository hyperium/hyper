#![deny(warnings)]
extern crate hyper;
extern crate futures;
extern crate pretty_env_logger;
extern crate tokio;

use futures::Future;

use hyper::{Body, Response};
use hyper::server::{Server, const_service, service_fn};

static PHRASE: &'static [u8] = b"Hello World!";

fn main() {
    pretty_env_logger::init();

    let addr = ([127, 0, 0, 1], 3000).into();

    let new_service = const_service(service_fn(|_| {
        //TODO: when `!` is stable, replace error type
        Ok::<_, hyper::Error>(Response::new(Body::from(PHRASE)))
    }));

    let server = Server::bind(&addr)
        .serve(new_service)
        .map_err(|e| eprintln!("server error: {}", e));

    println!("Listening on http://{}", addr);

    tokio::run(server);
}
