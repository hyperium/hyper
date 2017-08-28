#![feature(test)]
#![deny(warnings)]

extern crate futures;
extern crate hyper;
extern crate test;

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;

use futures::Future;
use futures::sync::oneshot;

use hyper::header::{ContentLength, ContentType, TransferEncoding};
use hyper::server::{self, Service};

macro_rules! bench_server {
    ($b:ident, $handler:ident, $total_bytes:expr) => {
        let (_until_tx, until_rx) = oneshot::channel();
        let addr = {
            let (addr_tx, addr_rx) = mpsc::channel();
            ::std::thread::spawn(move || {
                let addr = "127.0.0.1:0".parse().unwrap();
                let srv = hyper::server::Http::new().bind(&addr, || Ok($handler)).unwrap();
                let addr = srv.local_addr().unwrap();
                addr_tx.send(addr).unwrap();
                srv.run_until(until_rx.map_err(|_| ())).unwrap();
            });

            addr_rx.recv().unwrap()
        };

        let mut tcp = TcpStream::connect(addr).unwrap();
        let mut buf = [0u8; 8192];

        $b.bytes = 35 + $total_bytes;
        $b.iter(|| {
            tcp.write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n").unwrap();
            let mut sum = 0;
            while sum < $total_bytes {
                sum += tcp.read(&mut buf).unwrap();
            }
            assert_eq!(sum, $total_bytes);
        })
    }
}


#[bench]
fn bench_server_tcp_throughput_fixedsize_small_payload(b: &mut test::Bencher) {
    bench_server!(b, FixedSizeSmallPayload, 130);
}

#[bench]
fn bench_server_tcp_throughput_chunked_small_payload(b: &mut test::Bencher) {
    bench_server!(b, ChunkedSmallPayload, 148);
}

#[bench]
fn bench_server_tcp_throughput_chunked_large_payload(b: &mut test::Bencher) {
    bench_server!(b, ChunkedLargePayload, 13_000_140);
}

#[bench]
fn bench_raw_tcp_throughput_small_payload(b: &mut test::Bencher) {
    let (tx, rx) = mpsc::channel();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    ::std::thread::spawn(move || {
        let mut sock = listener.accept().unwrap().0;

        let mut buf = [0u8; 8192];
        while rx.try_recv().is_err() {
            sock.read(&mut buf).unwrap();
            sock.write_all(b"\
                HTTP/1.1 200 OK\r\n\
                Content-Length: 13\r\n\
                Content-Type: text/plain; charset=utf-8\r\n\
                Date: Fri, 12 May 2017 18:21:45 GMT\r\n\
                \r\n\
                Hello, World!\
            ").unwrap();
        }
    });

    let mut tcp = TcpStream::connect(addr).unwrap();
    let mut buf = [0u8; 4096];

    b.bytes = 130 + 35;
    b.iter(|| {
        tcp.write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n").unwrap();
        let n = tcp.read(&mut buf).unwrap();
        assert_eq!(n, 130);
    });
    tx.send(()).unwrap();
}

struct FixedSizeSmallPayload;

impl Service for FixedSizeSmallPayload {
    type Request = server::Request;
    type Response = server::Response;
    type Error = hyper::Error;
    type Future = ::futures::Finished<Self::Response, hyper::Error>;
    fn call(&self, _req: Self::Request) -> Self::Future {
        ::futures::finished(
            server::Response::new()
                .with_header(ContentLength("Hello, World!".len() as u64))
                .with_header(ContentType::plaintext())
                .with_body("Hello, World!")
        )
    }
}

struct ChunkedSmallPayload;

impl Service for ChunkedSmallPayload {
    type Request = server::Request;
    type Response = server::Response;
    type Error = hyper::Error;
    type Future = ::futures::Finished<Self::Response, hyper::Error>;
    fn call(&self, _req: Self::Request) -> Self::Future {
        ::futures::finished(
            server::Response::new()
                .with_header(TransferEncoding::chunked())
                .with_header(ContentType::plaintext())
                .with_body("Hello, World!")
        )
    }
}

struct ChunkedLargePayload;

impl Service for ChunkedLargePayload {
    type Request = server::Request;
    type Response = server::Response;
    type Error = hyper::Error;
    type Future = ::futures::Finished<Self::Response, hyper::Error>;
    fn call(&self, _req: Self::Request) -> Self::Future {
        ::futures::finished(
            server::Response::new()
                .with_header(TransferEncoding::chunked())
                .with_header(ContentType::plaintext())
                .with_body((0..1_000_000).flat_map(|_| "Hello, World!".bytes()).collect::<Vec<u8>>())
        )
    }
}

