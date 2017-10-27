//#![deny(warnings)]
extern crate futures;
extern crate hyper;
extern crate tokio_core;

extern crate pretty_env_logger;

use std::env;
use std::io::{self, Write};

use futures::Future;
use futures::stream::Stream;

use hyper::Client;

fn main() {
    pretty_env_logger::init().unwrap();

    let url = match env::args().nth(1) {
        Some(url) => url,
        None => {
            println!("Usage: client <url>");
            return;
        }
    };

    let url = url.parse::<hyper::Uri>().unwrap();
    if url.scheme() != Some("http") {
        println!("This example only works with 'http' URLs.");
        return;
    }

    let mut core = tokio_core::reactor::Core::new().unwrap();
    let handle = core.handle();
    let client = Client::configure()
        .no_proto()
        .build(&handle);

    let work = client.get(url).and_then(|res| {
        println!("Response: {}", res.status());
        println!("Headers: \n{}", res.headers());

        res.body().for_each(|chunk| {
            io::stdout().write_all(&chunk).map_err(From::from)
        })
    }).map(|_| {
        println!("\n\nDone.");
    });

    core.run(work).unwrap();
}
