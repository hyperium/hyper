#![feature(async_await)]
#![deny(warnings)]
extern crate hyper;
extern crate pretty_env_logger;

use std::env;
use std::io::{self, Write};

use hyper::Client;
use hyper::rt;

fn main() {
    pretty_env_logger::init();

    // Some simple CLI args requirements...
    let url = match env::args().nth(1) {
        Some(url) => url,
        None => {
            println!("Usage: client <url>");
            return;
        }
    };

    // HTTPS requires picking a TLS implementation, so give a better
    // warning if the user tries to request an 'https' URL.
    let url = url.parse::<hyper::Uri>().unwrap();
    if url.scheme_part().map(|s| s.as_ref()) != Some("http") {
        println!("This example only works with 'http' URLs.");
        return;
    }

    // Run the runtime with the future trying to fetch and print this URL.
    //
    // Note that in more complicated use cases, the runtime should probably
    // run on its own, and futures should just be spawned into it.
    rt::run(fetch_url(url));
}

async fn fetch_url(url: hyper::Uri) {
    let client = Client::new();

    let res = match client.get(url).await {
        Ok(res) => res,
        Err(err) => {
            eprintln!("Response Error: {}", err);
            return;
        }
    };

    println!("Response: {}", res.status());
    println!("Headers: {:#?}\n", res.headers());

    let mut body = res.into_body();

    while let Some(next) = body.next().await {
        match next {
            Ok(chunk) => {
                io::stdout().write_all(&chunk)
                    .expect("example expects stdout is open");
            },
            Err(err) => {
                eprintln!("Body Error: {}", err);
                return;
            }
        }
    }

    println!("\n\nDone!");
}
