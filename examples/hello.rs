#![feature(io, net)]
extern crate hyper;

use std::io::Write;
use std::net::IpAddr;
use hyper::server::{Request, Response};

static PHRASE: &'static [u8] = b"Hello World!";

fn hello(_: Request, res: Response) {
    let mut res = res.start().unwrap();
    res.write_all(PHRASE).unwrap();
    res.end().unwrap();
}

fn main() {
    let _listening = hyper::Server::http(hello)
        .listen(IpAddr::new_v4(127, 0, 0, 1), 3000).unwrap();
    println!("Listening on http://127.0.0.1:3000");
}
