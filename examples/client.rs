extern crate hyper;

extern crate env_logger;

use std::env;
use std::io::{Read,};  // enables read_to_string

use hyper::Client;
use hyper::client::response::Response;
use hyper::header::Connection;


// add more parameters here to support adding custom headers
fn process(client: &Client, url: &str) -> hyper::Result<Response> {
    client.get(url)
          .header(Connection::close())
          .send()
}

fn main() {
    env_logger::init().unwrap();

    let url = match env::args().nth(1) {
        Some(url) => url,
        None => {
            println!("Usage: client <url>");
            return;
        }
    };

    let client = Client::new();

    let mut res = match process(&client, &url) {
        Ok(res) => res,
        Err(e) => {
            // this will be things like name server lookup failure
            println!("Error: {}", e);
            return;
        }
    };

    /*
     * the Request was handled by the HTTP server.
     *
     * Check res.status to find out if it was successful.
     * Inspect the headers to see what data type was returned and then
     * parse the response data stored in `contents`.
     */
    println!("Response: {}", res.status);
    println!("Headers:\n{}", res.headers);

    let mut contents = String::new();
    res.read_to_string(&mut contents).unwrap();

    println!("Contents: {}", contents);
}
