#![feature(test)]
#![deny(warnings)]

extern crate futures;
extern crate hyper;
extern crate test;
extern crate tokio_core;

use std::net::SocketAddr;

use futures::{future, Future, Stream};
use tokio_core::reactor::{Core, Handle};
use tokio_core::net::TcpListener;

use hyper::client;
use hyper::header::{ContentLength, ContentType};
use hyper::Method;
use hyper::server::{self, Service};


#[bench]
fn get_one_at_a_time(b: &mut test::Bencher) {
    let mut core = Core::new().unwrap();
    let handle = core.handle();
    let addr = spawn_hello(&handle);

    let client = hyper::Client::new(&handle);

    let url: hyper::Uri = format!("http://{}/get", addr).parse().unwrap();

    b.bytes = 160 * 2 + PHRASE.len() as u64;
    b.iter(move || {
        let work = client.get(url.clone()).and_then(|res| {
            res.body().for_each(|_chunk| {
                Ok(())
            })
        });

        core.run(work).unwrap();
    });
}

#[bench]
fn post_one_at_a_time(b: &mut test::Bencher) {
    let mut core = Core::new().unwrap();
    let handle = core.handle();
    let addr = spawn_hello(&handle);

    let client = hyper::Client::new(&handle);

    let url: hyper::Uri = format!("http://{}/get", addr).parse().unwrap();

    let post = "foo bar baz quux";
    b.bytes = 180 * 2 + post.len() as u64 + PHRASE.len() as u64;
    b.iter(move || {
        let mut req = client::Request::new(Method::Post, url.clone());
        req.headers_mut().set(ContentLength(post.len() as u64));
        req.set_body(post);

        let work = client.request(req).and_then(|res| {
            res.body().for_each(|_chunk| {
                Ok(())
            })
        });

        core.run(work).unwrap();
    });
}

static PHRASE: &'static [u8] = include_bytes!("../CHANGELOG.md"); //b"Hello, World!";

#[derive(Clone, Copy)]
struct Hello;

impl Service for Hello {
    type Request = server::Request;
    type Response = server::Response;
    type Error = hyper::Error;
    type Future = future::FutureResult<Self::Response, hyper::Error>;
    fn call(&self, _req: Self::Request) -> Self::Future {
        future::ok(
            server::Response::new()
                .with_header(ContentLength(PHRASE.len() as u64))
                .with_header(ContentType::plaintext())
                .with_body(PHRASE)
        )
    }

}

fn spawn_hello(handle: &Handle) -> SocketAddr {
    let addr = "127.0.0.1:0".parse().unwrap();
    let listener = TcpListener::bind(&addr, handle).unwrap();
    let addr = listener.local_addr().unwrap();

    let handle2 = handle.clone();
    let http = hyper::server::Http::<hyper::Chunk>::new();
    handle.spawn(listener.incoming().for_each(move |(socket, _addr)| {
        handle2.spawn(
            http.serve_connection(socket, Hello)
                .map(|_| ())
                .map_err(|_| ())
        );
        Ok(())
    }).then(|_| Ok(())));
    return addr
}
