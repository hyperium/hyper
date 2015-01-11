#![allow(unstable)]
extern crate hyper;

use std::os;
use std::io::stdout;
use std::io::util::copy;

use hyper::Client;

fn main() {
    let args = os::args();
    match args.len() {
        2 => (),
        _ => {
            println!("Usage: client <url>");
            return;
        }
    };

    let url = &*args[1];

    let mut client = Client::new();

    let mut res = match client.get(url).send() {
        Ok(res) => res,
        Err(err) => panic!("Failed to connect: {:?}", err)
    };

    println!("Response: {}", res.status);
    println!("Headers:\n{}", res.headers);
    match copy(&mut res, &mut stdout()) {
        Ok(..) => (),
        Err(e) => panic!("Stream failure: {:?}", e)
    };

}
