#![deny(warnings)]

use hyper::service::{make_service_fn, service_fn};
use hyper::{Client, Error, Server};
use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    pretty_env_logger::init();

    let in_addr = ([127, 0, 0, 1], 3001).into();
    let out_addr: SocketAddr = ([127, 0, 0, 1], 3000).into();

    let client_main = Client::new();

    let out_addr_clone = out_addr.clone();

    // The closure inside `make_service_fn` is run for each connection,
    // creating a 'service' to handle requests for that specific connection.
    let make_service = make_service_fn(move |_| {
        let client = client_main.clone();

        async move {
            // This is the `Service` that will handle the connection.
            // `service_fn` is a helper to convert a function that
            // returns a Response into a `Service`.
            Ok::<_, Error>(service_fn(move |mut req| {
                let uri_string = format!(
                    "http://{}{}",
                    out_addr_clone,
                    req.uri()
                        .path_and_query()
                        .map(|x| x.as_str())
                        .unwrap_or("/")
                );
                let uri = uri_string.parse().unwrap();
                *req.uri_mut() = uri;
                client.request(req)
            }))
        }
    });

    let server = Server::bind(&in_addr).serve(make_service);

    println!("Listening on http://{}", in_addr);
    println!("Proxying on http://{}", out_addr);

    if let Err(e) = server.await {
        eprintln!("server error: {}", e);
    }
}
