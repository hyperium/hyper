#![deny(warnings)]
extern crate hyper;
extern crate tokio;

extern crate pretty_env_logger;

use std::env;
use std::io::{self, Write};

use hyper::Client;
use hyper::rt::{Future, Stream};
use tokio::runtime::Runtime;

fn main() {
    pretty_env_logger::init();

    // Pass URL as first argument, or use a default
    let url = match env::args().nth(1) {
        Some(url) => url,
        None => "http://www.columbia.edu/~fdc/sample.html".to_owned()
    };

    // HTTPS requires picking a TLS implementation, so give a better warning
    // if the user tries to request an 'https' URL.
    let url = url.parse::<hyper::Uri>().unwrap();
    if url.scheme_part().map(|s| s.as_ref()) != Some("http") {
        println!("This example only works with 'http' URLs.");
        return;
    }

    let mut runtime = Runtime::new().unwrap();
    let client = Client::new();

    let job = client.get(url) // HTTP GET request on URL
        .and_then(|res| {     // On successful (non-error) response
            println!("Response: {}", res.status());
            println!("Headers: {:#?}", res.headers());

            // The body is a stream, and for_each returns a new Future when
            // the stream is finished, and calls the closure on each chunk of
            // the body...
            res.into_body().for_each(|chunk| {
                io::stdout()
                    .write_all(&chunk)
                    .map_err(|e| {
                        panic!("example expects stdout is open, error={}", e)
                    })
            })
        })
        .map(|_| {           // When done (success)
            println!("\n\nDone.");
        })
        .map_err(|err| {     // When done (error)
            eprintln!("Error {}", err);
        });
    runtime.spawn(job); // non-blocking

    // Wait and shutdown sequence: drop `client` first, in order for "idle"
    // to occur as soon as `job` completes.
    drop(client);
    runtime.shutdown_on_idle().wait().unwrap();
}
