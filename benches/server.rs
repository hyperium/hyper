#![feature(test)]
#![deny(warnings)]

extern crate test;

mod support;

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::mpsc;
use std::time::Duration;

use bytes::Bytes;
use futures_util::{stream, StreamExt};
use http_body_util::{BodyExt, Full, StreamBody};
use tokio::sync::oneshot;

use hyper::body::Frame;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::Response;

macro_rules! bench_server {
    ($b:ident, $header:expr, $body:expr) => {{
        let _ = pretty_env_logger::try_init();
        let (_until_tx, until_rx) = oneshot::channel::<()>();

        let addr = {
            let (addr_tx, addr_rx) = mpsc::channel();
            std::thread::spawn(move || {
                let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("rt build");

                let listener = rt.block_on(tokio::net::TcpListener::bind(addr)).unwrap();
                let addr = listener.local_addr().unwrap();

                rt.spawn(async move {
                    loop {
                        let (stream, _) = listener.accept().await.expect("accept");
                        let io = support::TokioIo::new(stream);

                        http1::Builder::new()
                            .serve_connection(
                                io,
                                service_fn(|_| async {
                                    Ok::<_, hyper::Error>(
                                        Response::builder()
                                            .header($header.0, $header.1)
                                            .header("content-type", "text/plain")
                                            .body($body())
                                            .unwrap(),
                                    )
                                }),
                            )
                            .await
                            .unwrap();
                    }
                });

                addr_tx.send(addr).unwrap();
                rt.block_on(until_rx).ok();
            });

            addr_rx.recv().unwrap()
        };

        let total_bytes = {
            let mut tcp = TcpStream::connect(addr).unwrap();
            tcp.write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
                .unwrap();
            let mut buf = Vec::new();
            tcp.read_to_end(&mut buf).unwrap() - "connection: close\r\n".len()
        };

        let mut tcp = TcpStream::connect(addr).unwrap();
        tcp.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
        let mut buf = [0u8; 8192];

        $b.bytes = 35 + total_bytes as u64;
        $b.iter(|| {
            tcp.write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n")
                .unwrap();
            let mut sum = 0;
            while sum < total_bytes {
                sum += tcp.read(&mut buf).unwrap();
            }
            assert_eq!(sum, total_bytes);
        });
    }};
}

fn body(b: &'static [u8]) -> Full<Bytes> {
    Full::new(b.into())
}

#[bench]
fn throughput_fixedsize_small_payload(b: &mut test::Bencher) {
    bench_server!(b, ("content-length", "13"), || body(b"Hello, World!"))
}

#[bench]
fn throughput_fixedsize_large_payload(b: &mut test::Bencher) {
    bench_server!(b, ("content-length", "1000000"), || body(
        &[b'x'; 1_000_000]
    ))
}

#[bench]
fn throughput_fixedsize_many_chunks(b: &mut test::Bencher) {
    bench_server!(b, ("content-length", "1000000"), move || {
        static S: &[&[u8]] = &[&[b'x'; 1_000] as &[u8]; 1_000] as _;
        BodyExt::boxed(StreamBody::new(
            stream::iter(S.iter()).map(|&s| Ok::<_, String>(Frame::data(s))),
        ))
    })
}

#[bench]
fn throughput_chunked_small_payload(b: &mut test::Bencher) {
    bench_server!(b, ("transfer-encoding", "chunked"), || body(
        b"Hello, World!"
    ))
}

#[bench]
fn throughput_chunked_large_payload(b: &mut test::Bencher) {
    bench_server!(b, ("transfer-encoding", "chunked"), || body(
        &[b'x'; 1_000_000]
    ))
}

#[bench]
fn throughput_chunked_many_chunks(b: &mut test::Bencher) {
    bench_server!(b, ("transfer-encoding", "chunked"), || {
        static S: &[&[u8]] = &[&[b'x'; 1_000] as &[u8]; 1_000] as _;
        BodyExt::boxed(StreamBody::new(
            stream::iter(S.iter()).map(|&s| Ok::<_, String>(Frame::data(s))),
        ))
    })
}

#[bench]
fn raw_tcp_throughput_small_payload(b: &mut test::Bencher) {
    let (tx, rx) = mpsc::channel();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        let mut sock = listener.accept().unwrap().0;

        let mut buf = [0u8; 8192];
        while rx.try_recv().is_err() {
            sock.read(&mut buf).unwrap();
            sock.write_all(
                b"\
                HTTP/1.1 200 OK\r\n\
                Content-Length: 13\r\n\
                Content-Type: text/plain; charset=utf-8\r\n\
                Date: Fri, 12 May 2017 18:21:45 GMT\r\n\
                \r\n\
                Hello, World!\
            ",
            )
            .unwrap();
        }
    });

    let mut tcp = TcpStream::connect(addr).unwrap();
    let mut buf = [0u8; 4096];

    b.bytes = 130 + 35;
    b.iter(|| {
        tcp.write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .unwrap();
        let n = tcp.read(&mut buf).unwrap();
        assert_eq!(n, 130);
    });
    tx.send(()).unwrap();
}

#[bench]
fn raw_tcp_throughput_large_payload(b: &mut test::Bencher) {
    let (tx, rx) = mpsc::channel();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();

    let srv_head = b"\
        HTTP/1.1 200 OK\r\n\
        Content-Length: 1000000\r\n\
        Content-Type: text/plain; charset=utf-8\r\n\
        Date: Fri, 12 May 2017 18:21:45 GMT\r\n\
        \r\n\
    ";
    std::thread::spawn(move || {
        let mut sock = listener.accept().unwrap().0;

        let mut buf = [0u8; 8192];
        while rx.try_recv().is_err() {
            let r = sock.read(&mut buf).unwrap();
            extern crate test;
            if r == 0 {
                break;
            }
            sock.write_all(srv_head).unwrap();
            sock.write_all(&[b'x'; 1_000_000]).unwrap();
        }
    });

    let mut tcp = TcpStream::connect(addr).unwrap();
    let mut buf = [0u8; 8192];

    let expect_read = srv_head.len() + 1_000_000;
    b.bytes = expect_read as u64 + 35;

    b.iter(|| {
        tcp.write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .unwrap();
        let mut sum = 0;
        while sum < expect_read {
            sum += tcp.read(&mut buf).unwrap();
        }
        assert_eq!(sum, expect_read);
    });
    tx.send(()).unwrap();
}
