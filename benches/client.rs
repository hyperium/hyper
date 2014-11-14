#![feature(macro_rules)]
extern crate curl;
extern crate http;
extern crate hyper;

extern crate test;

use std::fmt::{mod, Show};
use std::io::net::ip::Ipv4Addr;
use hyper::server::{Incoming, Server};

fn listen() -> hyper::server::Listening {
    let server = Server::http(Ipv4Addr(127, 0, 0, 1), 0);
    server.listen(handle).unwrap()
}

macro_rules! try_continue(
    ($e:expr) => {{
        match $e {
            Ok(v) => v,
            Err(..) => continue
        }
    }})

fn handle(mut incoming: Incoming) {
    for conn in incoming {
        let (_, res) = try_continue!(conn.open());
        let mut res = try_continue!(res.start());
        try_continue!(res.write(b"Benchmarking hyper vs others!"))
        try_continue!(res.end());
    }
}


#[bench]
fn bench_curl(b: &mut test::Bencher) {
    let mut listening = listen();
    let s = format!("http://{}/", listening.socket);
    let url = s.as_slice();
    b.iter(|| {
        curl::http::handle()
            .get(url)
            .header("X-Foo", "Bar")
            .exec()
            .unwrap()
    });
    listening.close().unwrap()
}

#[deriving(Clone)]
struct Foo;

impl hyper::header::Header for Foo {
    fn header_name(_: Option<Foo>) -> &'static str {
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
    b.iter(|| {
        let mut req = hyper::client::Request::get(hyper::Url::parse(url).unwrap()).unwrap();
        req.headers_mut().set(Foo);

        req.start().unwrap()
            .send().unwrap()
            .read_to_string().unwrap()
    });
    listening.close().unwrap()
}

#[bench]
fn bench_http(b: &mut test::Bencher) {
    let mut listening = listen();
    let s = format!("http://{}/", listening.socket);
    let url = s.as_slice();
    b.iter(|| {
        let mut req: http::client::RequestWriter = http::client::RequestWriter::new(
            http::method::Get,
            hyper::Url::parse(url).unwrap()
        ).unwrap();
        req.headers.extensions.insert("x-foo".to_string(), "Bar".to_string());
        // cant unwrap because Err contains RequestWriter, which does not implement Show
        let mut res = match req.read_response() {
            Ok(res) => res,
            Err(..) => panic!("http response failed")
        };
        res.read_to_string().unwrap();
    });
    listening.close().unwrap()
}
