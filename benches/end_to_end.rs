#![feature(test)]
#![deny(warnings)]

extern crate futures;
extern crate hyper;
extern crate pretty_env_logger;
extern crate test;
extern crate tokio;

use std::net::SocketAddr;

use futures::{Future, Stream};
use tokio::runtime::current_thread::Runtime;
use tokio::net::TcpListener;

use hyper::{Body, Method, Request, Response, Version};
use hyper::client::HttpConnector;
use hyper::server::conn::Http;

#[bench]
fn http1_get(b: &mut test::Bencher) {
    bench_with(b, Version::HTTP_11, || {
        Request::new(Body::empty())
    });
}

#[bench]
fn http1_post(b: &mut test::Bencher) {
    bench_with(b, Version::HTTP_11, || {
        let mut req = Request::new("foo bar baz quux".into());
        *req.method_mut() = Method::POST;
        req
    });
}

#[bench]
fn http2_get(b: &mut test::Bencher) {
    bench_with(b, Version::HTTP_2, || {
        Request::new(Body::empty())
    });
}

#[bench]
fn http2_post(b: &mut test::Bencher) {
    bench_with(b, Version::HTTP_2, || {
        let mut req = Request::new("foo bar baz quux".into());
        *req.method_mut() = Method::POST;
        req
    });
}

fn bench_with<F>(b: &mut test::Bencher, version: Version, make_request: F)
where
    F: Fn() -> Request<Body>,
{
    let mut rt = Runtime::new().unwrap();
    let body = b"Hello";
    let addr = spawn_hello(&mut rt, body);

    let connector = HttpConnector::new(1);
    let client = hyper::Client::builder()
        .http2_only(version == Version::HTTP_2)
        .build::<_, Body>(connector);

    let url: hyper::Uri = format!("http://{}/hello", addr).parse().unwrap();

    b.bytes = body.len() as u64;
    b.iter(move || {
        let mut req = make_request();
        *req.uri_mut() = url.clone();
        rt.block_on(client.request(req).and_then(|res| {
            res.into_body().for_each(|_chunk| {
                Ok(())
            })
        })).expect("client wait");
    });
}

fn spawn_hello(rt: &mut Runtime, body: &'static [u8]) -> SocketAddr {
    use hyper::service::{service_fn};
    let addr = "127.0.0.1:0".parse().unwrap();
    let listener = TcpListener::bind(&addr).unwrap();
    let addr = listener.local_addr().unwrap();

    let http = Http::new();

    let service = service_fn(move |req: Request<Body>| {
        req.into_body()
            .concat2()
            .map(move |_| {
                Response::new(Body::from(body))
            })
    });

    // Specifically only accept 1 connection.
    let srv = listener.incoming()
        .into_future()
        .map_err(|(e, _inc)| panic!("accept error: {}", e))
        .and_then(move |(accepted, _inc)| {
            let socket = accepted.expect("accepted socket");
            http.serve_connection(socket, service)
                .map_err(|_| ())
        });
    rt.spawn(srv);
    return addr
}
