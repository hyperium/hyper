#![deny(warnings)]
extern crate hyper;
extern crate pretty_env_logger;
extern crate serde_json;

use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};

use hyper::{Body, Response, Server};
use hyper::service::service_fn_ok;
use hyper::rt::{self, Future};

fn main() {
    pretty_env_logger::init();

    let addr = ([127, 0, 0, 1], 3000).into();

    // For the most basic of state, we just share a counter, that increments
    // with each request, and we send its value back in the response.
    let counter = Arc::new(AtomicUsize::new(0));

    // new_service is run for each connection, creating a 'service'
    // to handle requests for that specific connection.
    let new_service = move || {
        // While the state was moved into the new_service closure,
        // we need to clone it here because this closure is called
        // once for every connection.
        //
        // Each connection could send multiple requests, so
        // the `Service` needs a clone to handle later requests.
        let counter = counter.clone();

        // This is the `Service` that will handle the connection.
        // `service_fn_ok` is a helper to convert a function that
        // returns a Response into a `Service`.
        //
        // If you wanted to return a `Future` of a `Response`, such as because
        // you wish to load data from a database or do other things, you
        // could change this to `service_fn` instead.
        service_fn_ok(move |_req| {
            // Get the current count, and also increment by 1, in a single
            // atomic operation.
            let count = counter.fetch_add(1, Ordering::AcqRel);
            Response::new(Body::from(format!("Request #{}", count)))
        })
    };

    let server = Server::bind(&addr)
        .serve(new_service)
        .map_err(|e| eprintln!("server error: {}", e));

    println!("Listening on http://{}", addr);

    rt::run(server);
}

