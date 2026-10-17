#![feature(test)]
#![deny(warnings)]

//! Compares the HTTP/1 write strategies at a range of response body sizes.
//! `flatten` and `queue` force one strategy on via `http1::Builder::writev`;
//! `auto` leaves the default in place. Used to pick `AUTO_FLATTEN_LIMIT` in
//! `src/proto/h1/io.rs`, and to check that `auto` tracks whichever of the two
//! is faster at each size.

extern crate test;
mod support;

use std::convert::Infallible;
use std::net::SocketAddr;

use http_body_util::{BodyExt, Full};
use hyper::service::service_fn;
use hyper::{Request, Response};

fn spawn_server(
    rt: &tokio::runtime::Runtime,
    writev: Option<bool>,
    body: &'static [u8],
) -> SocketAddr {
    use tokio::net::TcpListener;
    let addr = "127.0.0.1:0".parse::<std::net::SocketAddr>().unwrap();
    let listener = rt.block_on(async { TcpListener::bind(&addr).await.unwrap() });
    let addr = listener.local_addr().unwrap();
    rt.spawn(async move {
        while let Ok((sock, _)) = listener.accept().await {
            let io = support::TokioIo::new(sock);
            let mut builder = hyper::server::conn::http1::Builder::new();
            if let Some(w) = writev {
                builder.writev(w);
            }
            tokio::spawn(builder.serve_connection(
                io,
                service_fn(move |req: Request<hyper::body::Incoming>| async move {
                    let mut req_body = req.into_body();
                    while let Some(_chunk) = req_body.frame().await {}
                    Ok::<_, Infallible>(Response::new(Full::<bytes::Bytes>::from(body)))
                }),
            ));
        }
    });
    addr
}

fn bench(b: &mut test::Bencher, writev: Option<bool>, body: &'static [u8]) {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("rt build");

    b.bytes = body.len() as u64;

    let addr = spawn_server(&rt, writev, body);

    let mut client = rt.block_on(async {
        let tcp = tokio::net::TcpStream::connect(&addr).await.unwrap();
        let io = support::TokioIo::new(tcp);
        let (tx, conn) = hyper::client::conn::http1::Builder::new()
            .handshake::<_, http_body_util::Empty<bytes::Bytes>>(io)
            .await
            .unwrap();
        tokio::spawn(conn);
        tx
    });

    let url: hyper::Uri = format!("http://{}/hello", addr).parse().unwrap();

    b.iter(|| {
        let mut req = Request::new(http_body_util::Empty::<bytes::Bytes>::new());
        *req.uri_mut() = url.clone();
        rt.block_on(async {
            let res = client.send_request(req).await.expect("client wait");
            let mut body = res.into_body();
            while let Some(_chunk) = body.frame().await {}
        });
    });
}

macro_rules! sizes {
    ($($n:ident: $sz:expr,)*) => {
        $(
            mod $n {
                const BODY: &[u8] = &[b'x'; $sz];
                #[bench]
                fn auto(b: &mut ::test::Bencher) {
                    super::bench(b, None, BODY)
                }
                #[bench]
                fn flatten(b: &mut ::test::Bencher) {
                    super::bench(b, Some(false), BODY)
                }
                #[bench]
                fn queue(b: &mut ::test::Bencher) {
                    super::bench(b, Some(true), BODY)
                }
            }
        )*
    };
}

sizes! {
    b_00064: 64,
    b_00256: 256,
    b_01024: 1024,
    b_04096: 4096,
    b_08192: 8192,
    b_16384: 16384,
    b_20480: 20480,
    b_24576: 24576,
    b_32768: 32768,
    b_65536: 65536,
    b_262144: 262144,
    b_1048576: 1048576,
}
