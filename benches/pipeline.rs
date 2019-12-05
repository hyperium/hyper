#![feature(test)]
#![deny(warnings)]

extern crate test;

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::mpsc;
use std::time::Duration;

use tokio::sync::oneshot;

use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Response, Server};

const PIPELINED_REQUESTS: usize = 16;

#[bench]
fn hello_world(b: &mut test::Bencher) {
    let _ = pretty_env_logger::try_init();
    let (_until_tx, until_rx) = oneshot::channel::<()>();

    let addr = {
        let (addr_tx, addr_rx) = mpsc::channel();
        std::thread::spawn(move || {
            let addr = "127.0.0.1:0".parse().unwrap();

            let make_svc = make_service_fn(|_| {
                async {
                    Ok::<_, hyper::Error>(service_fn(|_| {
                        async { Ok::<_, hyper::Error>(Response::new(Body::from("Hello, World!"))) }
                    }))
                }
            });

            let mut rt = tokio::runtime::Builder::new()
                .enable_all()
                .basic_scheduler()
                .build()
                .expect("rt build");
            let srv = rt.block_on(async move {
                Server::bind(&addr)
                    .http1_pipeline_flush(true)
                    .serve(make_svc)
            });

            addr_tx.send(srv.local_addr()).unwrap();

            let graceful = srv.with_graceful_shutdown(async {
                until_rx.await.ok();
            });

            rt.block_on(async {
                if let Err(e) = graceful.await {
                    panic!("server error: {}", e);
                }
            });
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
