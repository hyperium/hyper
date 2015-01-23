#![allow(unstable)]
extern crate hyper;

extern crate test;

use std::fmt::{self, Show};
use std::io::net::ip::Ipv4Addr;
use hyper::server::{Request, Response, Server};
use hyper::header::Headers;
use hyper::Client;

fn listen() -> hyper::server::Listening {
    let server = Server::http(Ipv4Addr(127, 0, 0, 1), 0);
    server.listen(handle).unwrap()
}

macro_rules! try_return(
    ($e:expr) => {{
        match $e {
            Ok(v) => v,
            Err(..) => return
        }
    }}
);

fn handle(_r: Request, res: Response) {
    static BODY: &'static [u8] = b"Benchmarking hyper vs others!";
    let mut res = try_return!(res.start());
    try_return!(res.write(BODY));
    try_return!(res.end());
}

#[derive(Clone)]
struct Foo;

impl hyper::header::Header for Foo {
    fn header_name() -> &'static str {
        "x-foo"
    }
    fn parse_header(_: &[Vec<u8>]) -> Option<Foo> {
        None
    }
}

impl hyper::header::HeaderFormat for Foo {
    fn fmt_header(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        "Bar".fmt(fmt)
    }
}

#[bench]
fn bench_hyper(b: &mut test::Bencher) {
    let mut listening = listen();
    let s = format!("http://{}/", listening.socket);
    let url = s.as_slice();
    let mut client = Client::new();
    let mut headers = Headers::new();
    headers.set(Foo);
    b.iter(|| {
        client.get(url).header(Foo).send().unwrap().read_to_string().unwrap();
    });
    listening.close().unwrap()
}

