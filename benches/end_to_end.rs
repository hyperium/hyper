#![feature(test)]
#![deny(warnings)]

extern crate futures;
extern crate hyper;
extern crate test;
extern crate tokio;

use std::net::SocketAddr;

use futures::{FutureExt, StreamExt};
use futures::executor::block_on;
use tokio::runtime::Runtime;
use tokio::net::TcpListener;

use hyper::{Body, Method, Request, Response};
use hyper::server::Http;


#[bench]
fn get_one_at_a_time(b: &mut test::Bencher) {
    let mut rt = Runtime::new().unwrap();
    let addr = spawn_hello(&mut rt);

    let client = hyper::Client::configure()
        .build(&rt.handle());

    let url: hyper::Uri = format!("http://{}/get", addr).parse().unwrap();

    b.bytes = 160 * 2 + PHRASE.len() as u64;
    b.iter(move || {
        block_on(client.get(url.clone())
            .with_executor(rt.executor())
            .and_then(|res| {
                res.into_body().into_stream().for_each(|_chunk| {
                    Ok(())
                }).map(|_| ())
            })
        ).expect("client wait");
    });
}

#[bench]
fn post_one_at_a_time(b: &mut test::Bencher) {
    let mut rt = Runtime::new().unwrap();
    let addr = spawn_hello(&mut rt);

    let client = hyper::Client::configure()
        .build(&rt.handle());

    let url: hyper::Uri = format!("http://{}/post", addr).parse().unwrap();

    let post = "foo bar baz quux";
    b.bytes = 180 * 2 + post.len() as u64 + PHRASE.len() as u64;
    b.iter(move || {
        let mut req = Request::new(post.into());
        *req.method_mut() = Method::POST;
        *req.uri_mut() = url.clone();
        block_on(client.request(req)
            .with_executor(rt.executor())
            .and_then(|res| {
                res.into_body().into_stream().for_each(|_chunk| {
                    Ok(())
                }).map(|_| ())
            })
        ).expect("client wait");

    });
}

static PHRASE: &'static [u8] = include_bytes!("../CHANGELOG.md"); //b"Hello, World!";

fn spawn_hello(rt: &mut Runtime) -> SocketAddr {
    use hyper::server::{const_service, service_fn, NewService};
    let addr = "127.0.0.1:0".parse().unwrap();
    let listener = TcpListener::bind(&addr).unwrap();
    let addr = listener.local_addr().unwrap();

    let http = Http::<hyper::Chunk>::new();

    let service = const_service(service_fn(|req: Request<Body>| {
        req.into_body()
            .into_stream()
            .concat()
            .map(|_| {
                Response::new(Body::from(PHRASE))
            })
    }));

    let srv = listener.incoming()
        .next()
        .map_err(|(e, _inc)| panic!("accept error: {}", e))
        .and_then(move |(accepted, _inc)| {
            let socket = accepted.expect("accepted socket");
            http.serve_connection(socket, service.new_service().expect("new_service"))
                .map(|_| ())
                .map_err(|_| ())
        })
        .map_err(|_| panic!("server error"));
    rt.spawn2(srv);
    return addr
}
