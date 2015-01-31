#![feature(io)]
extern crate hyper;

use std::old_io::net::ip::Ipv4Addr;
use hyper::server::{Request, Response};

static PHRASE: &'static [u8] = b"Hello World!";

fn hello(_: Request, res: Response) {
    let mut res = res.start().unwrap();
    res.write_all(PHRASE).unwrap();
    res.end().unwrap();
}

fn main() {
    hyper::Server::http(Ipv4Addr(127, 0, 0, 1), 3000).listen(hello).unwrap();
    println!("Listening on http://127.0.0.1:3000");
}
