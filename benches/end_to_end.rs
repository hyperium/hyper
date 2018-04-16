#![feature(test)]
#![deny(warnings)]

extern crate futures;
extern crate hyper;
extern crate test;
extern crate tokio;

use std::net::SocketAddr;

use futures::{Future, Stream};
use tokio::runtime::Runtime;
use tokio::net::TcpListener;

use hyper::{Body, Method, Request, Response};
use hyper::client::HttpConnector;
use hyper::server::conn::Http;


#[bench]
fn get_one_at_a_time(b: &mut test::Bencher) {
    let mut rt = Runtime::new().unwrap();
    let addr = spawn_hello(&mut rt);

    let connector = HttpConnector::new_with_handle(1, rt.reactor().clone());
    let client = hyper::Client::builder()
        .executor(rt.executor())
        .build::<_, Body>(connector);

    let url: hyper::Uri = format!("http://{}/get", addr).parse().unwrap();

    b.bytes = 160 * 2 + PHRASE.len() as u64;
    b.iter(move || {
        client.get(url.clone())
            .and_then(|res| {
                res.into_body().for_each(|_chunk| {
                    Ok(())
                })
            })
            .wait().expect("client wait");
    });
}

#[bench]
fn post_one_at_a_time(b: &mut test::Bencher) {
    let mut rt = Runtime::new().unwrap();
    let addr = spawn_hello(&mut rt);

    let connector = HttpConnector::new_with_handle(1, rt.reactor().clone());
    let client = hyper::Client::builder()
        .executor(rt.executor())
        .build::<_, Body>(connector);

    let url: hyper::Uri = format!("http://{}/post", addr).parse().unwrap();

    let post = "foo bar baz quux";
    b.bytes = 180 * 2 + post.len() as u64 + PHRASE.len() as u64;
    b.iter(move || {
        let mut req = Request::new(post.into());
        *req.method_mut() = Method::POST;
        *req.uri_mut() = url.clone();
        client.request(req).and_then(|res| {
            res.into_body().for_each(|_chunk| {
                Ok(())
            })
        }).wait().expect("client wait");

    });
}

static PHRASE: &'static [u8] = include_bytes!("../CHANGELOG.md"); //b"Hello, World!";

fn spawn_hello(rt: &mut Runtime) -> SocketAddr {
    use hyper::server::{const_service, service_fn, NewService};
    let addr = "127.0.0.1:0".parse().unwrap();
    let listener = TcpListener::bind(&addr).unwrap();
    let addr = listener.local_addr().unwrap();

    let http = Http::new();

    let service = const_service(service_fn(|req: Request<Body>| {
        req.into_body()
            .concat2()
            .map(|_| {
                Response::new(Body::from(PHRASE))
            })
    }));

    // Specifically only accept 1 connection.
    let srv = listener.incoming()
        .into_future()
        .map_err(|(e, _inc)| panic!("accept error: {}", e))
        .and_then(move |(accepted, _inc)| {
            let socket = accepted.expect("accepted socket");
            http.serve_connection(socket, service.new_service().expect("new_service"))
                .map(|_| ())
                .map_err(|_| ())
        });
    rt.spawn(srv);
    return addr
}
