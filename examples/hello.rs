#![deny(warnings)]
extern crate hyper;
extern crate env_logger;

use std::io::Write;
use std::net::Ipv4Addr;
use hyper::server::{Request, Response};

static PHRASE: &'static [u8] = b"Hello World!";

fn hello(_: Request, res: Response) {
    let mut res = res.start().unwrap();
    res.write_all(PHRASE).unwrap();
    res.end().unwrap();
}

fn main() {
    env_logger::init().unwrap();
    let _listening = hyper::Server::http(hello)
        .listen(Ipv4Addr::new(127, 0, 0, 1), 3000).unwrap();
    println!("Listening on http://127.0.0.1:3000");
}
