#![deny(warnings)]
extern crate hyper;

extern crate env_logger;

use std::env;
use std::io;

use hyper::Client;
use hyper::header::Connection;

fn main() {
    env_logger::init().unwrap();

    let url = match env::args().nth(1) {
        Some(url) => url,
        None => {
            println!("Usage: client <url>");
            return;
        }
    };

    let client = match env::var("HTTP_PROXY") {
        Ok(mut proxy) => {
            // parse the proxy, message if it doesn't make sense
            let mut port = 80;
            if let Some(colon) = proxy.rfind(':') {
                port = proxy[colon + 1..].parse().unwrap_or_else(|e| {
                    panic!("HTTP_PROXY is malformed: {:?}, port parse error: {}", proxy, e);
                });
                proxy.truncate(colon);
            }
            Client::with_http_proxy(proxy, port)
        },
        _ => Client::new()
    };

    let mut res = client.get(&*url)
        .header(Connection::close())
        .send().unwrap();

    println!("Response: {}", res.status);
    println!("Headers:\n{}", res.headers);
    io::copy(&mut res, &mut io::stdout()).unwrap();
}
