#![deny(warnings)]
extern crate futures;
extern crate hyper;
extern crate pretty_env_logger;
extern crate tokio;

use std::cell::Cell;
use std::rc::Rc;

use hyper::{Body, Response, Server};
use hyper::service::service_fn_ok;
use hyper::rt::Future;
use tokio::runtime::current_thread;

fn main() {
    pretty_env_logger::init();

    let addr = ([127, 0, 0, 1], 3000).into();

    // Using a !Send request counter is fine on 1 thread...
    let counter = Rc::new(Cell::new(0));

    let new_service = move || {
        // For each connection, clone the counter to use in our service...
        let cnt = counter.clone();

        service_fn_ok(move |_| {
            let prev = cnt.get();
            cnt.set(prev + 1);
            Response::new(Body::from(format!("Request count: {}", prev + 1)))
        })
    };

    // Since the Server needs to spawn some background tasks, we needed
    // to configure an Executor that can spawn !Send futures...
    let exec = current_thread::TaskExecutor::current();

    let server = Server::bind(&addr)
        .executor(exec)
        .serve(new_service)
        .map_err(|e| eprintln!("server error: {}", e));

    println!("Listening on http://{}", addr);

    current_thread::Runtime::new()
        .expect("rt new")
        .spawn(server)
        .run()
        .expect("rt run");
}

