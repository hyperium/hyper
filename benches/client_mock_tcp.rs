#![feature(default_type_params)]
extern crate curl;
extern crate http;
extern crate hyper;

extern crate test;

use std::fmt::{mod, Show};
use std::from_str::from_str;
use std::io::{IoResult, MemReader};
use std::io::net::ip::SocketAddr;
use std::os;
use std::path::BytesContainer;

use http::connecter::Connecter;

use hyper::net;

static README: &'static [u8] = include_bin!("../README.md");


struct MockStream {
    read: MemReader,
}

impl Clone for MockStream {
    fn clone(&self) -> MockStream {
        MockStream::new()
    }
}

impl MockStream {
    fn new() -> MockStream {
        let head = b"HTTP/1.1 200 OK\r\nServer: Mock\r\n\r\n";
        let mut res = Vec::from_slice(head);
        res.push_all(README);
        MockStream {
            read: MemReader::new(res),
        }
    }
}

impl Reader for MockStream {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<uint> {
        self.read.read(buf)
    }
}

impl Writer for MockStream {
    fn write(&mut self, _msg: &[u8]) -> IoResult<()> {
        // we're mocking, what do we care.
        Ok(())
    }
}

#[bench]
fn bench_mock_curl(b: &mut test::Bencher) {
    let mut cwd = os::getcwd();
    cwd.push("README.md");
    let s = format!("file://{}", cwd.container_as_str().unwrap());
    let url = s.as_slice();
    b.iter(|| {
        curl::http::handle()
            .get(url)
            .header("X-Foo", "Bar")
            .exec()
            .unwrap()
    });
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

impl net::NetworkStream for MockStream {

    fn connect(_host: &str, _port: u16, _scheme: &str) -> IoResult<MockStream> {
        Ok(MockStream::new())
    }

    fn peer_name(&mut self) -> IoResult<SocketAddr> {
        Ok(from_str("127.0.0.1:1337").unwrap())
    }
}

#[bench]
fn bench_mock_hyper(b: &mut test::Bencher) {
    let url = "http://127.0.0.1:1337/";
    b.iter(|| {
        let mut req = hyper::client::Request::with_stream::<MockStream>(
            hyper::Get, hyper::Url::parse(url).unwrap()).unwrap();
        req.headers_mut().set(Foo);

        req
            .start().unwrap()
            .send().unwrap()
            .read_to_string().unwrap()
    });
}

impl Connecter for MockStream {
    fn connect(_addr: SocketAddr, _host: &str, _use_ssl: bool) -> IoResult<MockStream> {
        Ok(MockStream::new())
    }
}

#[bench]
fn bench_mock_http(b: &mut test::Bencher) {
    let url = "http://127.0.0.1:1337/";
    b.iter(|| {
        let mut req: http::client::RequestWriter<MockStream> = http::client::RequestWriter::new(
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
}

