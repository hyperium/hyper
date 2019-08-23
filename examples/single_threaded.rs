#![deny(warnings)]

use std::cell::Cell;
use std::rc::Rc;

use hyper::{Body, Error, Response, Server};
use hyper::service::{make_service_fn, service_fn};
use tokio::runtime::current_thread;

// Configure a runtime that runs everything on the current thread,
// which means it can spawn !Send futures...
#[tokio::main(single_thread)]
async fn main() {
    pretty_env_logger::init();

    let addr = ([127, 0, 0, 1], 3000).into();

    // Using a !Send request counter is fine on 1 thread...
    let counter = Rc::new(Cell::new(0));

    let make_service = make_service_fn(move |_| {
        // For each connection, clone the counter to use in our service...
        let cnt = counter.clone();

        async move {
            Ok::<_, Error>(service_fn(move |_| {
                let prev = cnt.get();
                cnt.set(prev + 1);
                let value = cnt.get();
                async move {
                    Ok::<_, Error>(Response::new(Body::from(
                        format!("Request #{}", value)
                    )))
                }
            }))
        }
    });

    // Since the Server needs to spawn some background tasks, we needed
    // to configure an Executor that can spawn !Send futures...
    let exec = current_thread::TaskExecutor::current();

    let server = Server::bind(&addr)
        .executor(exec)
        .serve(make_service);

    println!("Listening on http://{}", addr);

    // The server would block on current thread to await !Send futures.
    if let Err(e) = server.await {
        eprintln!("server error: {}", e);
    }
}

