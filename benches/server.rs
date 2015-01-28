#![allow(unstable)]
extern crate hyper;
extern crate test;

use test::Bencher;
use std::old_io::net::ip::Ipv4Addr;

use hyper::method::Method::Get;
use hyper::server::{Request, Response};

static PHRASE: &'static [u8] = b"Benchmarking hyper vs others!";

fn request(url: hyper::Url) {
    let req = hyper::client::Request::new(Get, url).unwrap();
    req.start().unwrap().send().unwrap().read_to_string().unwrap();
}

fn hyper_handle(_: Request, res: Response) {
    let mut res = res.start().unwrap();
    res.write_all(PHRASE).unwrap();
    res.end().unwrap();
}

#[bench]
fn bench_hyper(b: &mut Bencher) {
    let server = hyper::Server::http(Ipv4Addr(127, 0, 0, 1), 0);
    let mut listener = server.listen(hyper_handle).unwrap();

    let url = hyper::Url::parse(format!("http://{}", listener.socket).as_slice()).unwrap();
    b.iter(|| request(url.clone()));
    listener.close().unwrap();
}

