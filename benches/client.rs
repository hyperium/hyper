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

fn handle(mut incoming: Incoming) {
    for (_, mut res) in incoming {
        res.write(b"Benchmarking hyper vs others!").unwrap();
        res.end().unwrap();
    }
}


#[bench]
fn bench_curl(b: &mut test::Bencher) {
    let listening = listen();
    let s = format!("http://{}/", listening.socket_addr);
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

struct Foo;

impl hyper::header::Header for Foo {
    fn header_name(_: Option<Foo>) -> &'static str {
        "x-foo"
    }
    fn parse_header(_: &[Vec<u8>]) -> Option<Foo> {
        None
    }
    fn fmt_header(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        "Bar".fmt(fmt)
    }
}

#[bench]
fn bench_hyper(b: &mut test::Bencher) {
    let listening = listen();
    let s = format!("http://{}/", listening.socket_addr);
    let url = s.as_slice();
    b.iter(|| {
        let mut req = hyper::get(hyper::Url::parse(url).unwrap()).unwrap();
        req.headers.set(Foo);

        req
            .send().unwrap()
            .read_to_string().unwrap()
    });
    listening.close().unwrap()
}

#[bench]
fn bench_http(b: &mut test::Bencher) {
    let listening = listen();
    let s = format!("http://{}/", listening.socket_addr);
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
            Err(..) => fail!("http response failed")
        };
        res.read_to_string().unwrap();
    });
    listening.close().unwrap()
}
