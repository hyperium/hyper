#![feature(test)]
#![deny(warnings)]

extern crate test;

use std::convert::Infallible;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::mpsc;
use std::time::Duration;

use bytes::Bytes;
use http_body_util::Full;
use tokio::net::TcpListener;
use tokio::sync::oneshot;

use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::Response;

const PIPELINED_REQUESTS: usize = 16;

#[bench]
fn hello_world_16(b: &mut test::Bencher) {
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

            let listener = rt.block_on(TcpListener::bind(addr)).unwrap();
            let addr = listener.local_addr().unwrap();

            rt.spawn(async move {
                loop {
                    let (stream, _addr) = listener.accept().await.expect("accept");

                    http1::Builder::new()
                        .pipeline_flush(true)
                        .serve_connection(
                            stream,
                            service_fn(|_| async {
                                Ok::<_, Infallible>(Response::new(Full::new(Bytes::from(
                                    "Hello, World!",
                                ))))
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

    let mut pipelined_reqs = Vec::new();
    for _ in 0..PIPELINED_REQUESTS {
        pipelined_reqs.extend_from_slice(b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n");
    }

    let total_bytes = {
        let mut tcp = TcpStream::connect(addr).unwrap();
        tcp.write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
            .unwrap();
        let mut buf = Vec::new();
        tcp.read_to_end(&mut buf).unwrap()
    } * PIPELINED_REQUESTS;

    let mut tcp = TcpStream::connect(addr).unwrap();
    tcp.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    let mut buf = [0u8; 8192];

    b.bytes = (pipelined_reqs.len() + total_bytes) as u64;
    b.iter(|| {
        tcp.write_all(&pipelined_reqs).unwrap();
        let mut sum = 0;
        while sum < total_bytes {
            sum += tcp.read(&mut buf).unwrap();
        }
        assert_eq!(sum, total_bytes);
    });
}
