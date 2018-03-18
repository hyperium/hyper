//#![deny(warnings)]
extern crate futures;
extern crate hyper;
extern crate tokio;

extern crate pretty_env_logger;

use std::env;
use std::io::{self, Write};

use futures::{FutureExt, StreamExt};
use futures::future::lazy;

use hyper::{Body, Client, Request};

fn main() {
    pretty_env_logger::init();

    let url = match env::args().nth(1) {
        Some(url) => url,
        None => {
            println!("Usage: client <url>");
            return;
        }
    };

    let url = url.parse::<hyper::Uri>().unwrap();
    if url.scheme_part().map(|s| s.as_ref()) != Some("http") {
        println!("This example only works with 'http' URLs.");
        return;
    }

    tokio::runtime::run2(lazy(move |_| {
        let client = Client::default();

        let mut req = Request::new(Body::empty());
        *req.uri_mut() = url;

        client.request(req).and_then(|res| {
            println!("Response: {}", res.status());
            println!("Headers: {:#?}", res.headers());

            res.into_parts().1.into_stream().for_each(|chunk| {
                io::stdout().write_all(&chunk).map_err(From::from)
            })
        }).then(|result| {
            if let Some(err) = result.err() {
                eprintln!("Error {}", err);
            } else {
                println!("\n\nDone.");
            }
            Ok(())
        })
    }));
}
