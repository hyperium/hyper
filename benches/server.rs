#![deny(warnings)]
#![feature(net, test)]
extern crate hyper;
extern crate test;

use test::Bencher;
use std::io::{Read, Write};
use std::net::Ipv4Addr;

use hyper::method::Method::Get;
use hyper::server::{Request, Response};

static PHRASE: &'static [u8] = b"Benchmarking hyper vs others!";

fn request(url: hyper::Url) {
    let req = hyper::client::Request::new(Get, url).unwrap();
    let mut s = String::new();
    req.start().unwrap().send().unwrap().read_to_string(&mut s).unwrap();
}

fn hyper_handle(_: Request, res: Response) {
    let mut res = res.start().unwrap();
    res.write_all(PHRASE).unwrap();
    res.end().unwrap();
}

#[bench]
fn bench_hyper(b: &mut Bencher) {
    let server = hyper::Server::http(hyper_handle);
    let mut listener = server.listen(Ipv4Addr::new(127, 0, 0, 1), 0).unwrap();

    let url = hyper::Url::parse(&*format!("http://{}", listener.socket)).unwrap();
    b.iter(|| request(url.clone()));
    listener.close().unwrap();
}

