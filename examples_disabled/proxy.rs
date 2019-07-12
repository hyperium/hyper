#![deny(warnings)]
extern crate hyper;
extern crate pretty_env_logger;

use hyper::{Client, Server};
use hyper::service::service_fn;
use hyper::rt::{self, Future};
use std::net::SocketAddr;

fn main() {
    pretty_env_logger::init();

    let in_addr = ([127, 0, 0, 1], 3001).into();
    let out_addr: SocketAddr = ([127, 0, 0, 1], 3000).into();

    let client_main = Client::new();

    let out_addr_clone = out_addr.clone();
    // new_service is run for each connection, creating a 'service'
    // to handle requests for that specific connection.
    let new_service = move || {
        let client = client_main.clone();
        // This is the `Service` that will handle the connection.
        // `service_fn_ok` is a helper to convert a function that
        // returns a Response into a `Service`.
        service_fn(move |mut req| {
            let uri_string = format!("http://{}/{}",
                out_addr_clone,
                req.uri().path_and_query().map(|x| x.as_str()).unwrap_or(""));
            let uri = uri_string.parse().unwrap();
            *req.uri_mut() = uri;
            client.request(req)
        })
    };

    let server = Server::bind(&in_addr)
        .serve(new_service)
        .map_err(|e| eprintln!("server error: {}", e));

    println!("Listening on http://{}", in_addr);
    println!("Proxying on http://{}", out_addr);

    rt::run(server);
}
