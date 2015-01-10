#![allow(unstable)]
extern crate hyper;

extern crate test;

use std::fmt::{self, Show};
use std::io::{IoResult, MemReader};
use std::io::net::ip::SocketAddr;
use std::os;
use std::path::BytesContainer;

use hyper::net;

static README: &'static [u8] = include_bytes!("../README.md");


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
        let mut res = head.to_vec();
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

#[derive(Clone)]
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

impl net::NetworkStream for MockStream {
    fn peer_name(&mut self) -> IoResult<SocketAddr> {
        Ok("127.0.0.1:1337".parse().unwrap())
    }
}

struct MockConnector;

impl net::NetworkConnector for MockConnector {
    type Stream = MockStream;
    fn connect(&mut self, _: &str, _: u16, _: &str) -> IoResult<MockStream> {
        Ok(MockStream::new())
    }

}

#[bench]
fn bench_mock_hyper(b: &mut test::Bencher) {
    let url = "http://127.0.0.1:1337/";
    b.iter(|| {
        let mut req = hyper::client::Request::with_connector(
            hyper::Get, hyper::Url::parse(url).unwrap(), &mut MockConnector
        ).unwrap();
        req.headers_mut().set(Foo);

        req
            .start().unwrap()
            .send().unwrap()
            .read_to_string().unwrap()
    });
}

