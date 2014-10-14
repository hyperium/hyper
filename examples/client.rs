extern crate hyper;

use std::os;
use std::io::stdout;
use std::io::util::copy;

use hyper::Url;
use hyper::client::Request;

fn main() {
    let args = os::args();
    match args.len() {
        2 => (),
        _ => {
            println!("Usage: client <url>");
            return;
        }
    };

    let url = match Url::parse(args[1].as_slice()) {
        Ok(url) => {
            println!("GET {}...", url)
            url
        },
        Err(e) => fail!("Invalid URL: {}", e)
    };


    let req = match Request::get(url) {
        Ok(req) => req,
        Err(err) => fail!("Failed to connect: {}", err)
    };

    let mut res = req
        .start().unwrap() // failure: Error writing Headers
        .send().unwrap(); // failure: Error reading Response head.

    println!("Response: {}", res.status);
    println!("{}", res.headers);
    match copy(&mut res, &mut stdout()) {
        Ok(..) => (),
        Err(e) => fail!("Stream failure: {}", e)
    };

}
