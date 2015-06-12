#![deny(warnings)]
extern crate hyper;

extern crate env_logger;

use std::env;
use std::io;

use hyper::Client;
use hyper::header::Connection;
use hyper::http::h2;

fn main() {
    env_logger::init().unwrap();

    let url = match env::args().nth(1) {
        Some(url) => url,
        None => {
            println!("Usage: client <url>");
            return;
        }
    };

    let client = Client::with_protocol(h2::new_protocol());

    // `Connection: Close` is not a valid header for HTTP/2, but the client handles it gracefully.
    let mut res = client.get(&*url)
        .header(Connection::close())
        .send().unwrap();

    println!("Response: {}", res.status);
    println!("Headers:\n{}", res.headers);
    io::copy(&mut res, &mut io::stdout()).unwrap();
}
