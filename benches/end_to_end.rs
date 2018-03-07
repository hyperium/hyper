#![feature(test)]
#![deny(warnings)]

extern crate futures;
extern crate hyper;
extern crate test;
extern crate tokio_core;

use std::net::SocketAddr;

use futures::{Future, Stream};
use tokio_core::reactor::{Core, Handle};
use tokio_core::net::TcpListener;

use hyper::client;
use hyper::header::{ContentLength, ContentType};
use hyper::Method;


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
    extern crate pretty_env_logger;
    let _ = pretty_env_logger::try_init();
    let mut core = Core::new().unwrap();
    let handle = core.handle();
    let addr = spawn_hello(&handle);

    let client = hyper::Client::new(&handle);

    let url: hyper::Uri = format!("http://{}/post", addr).parse().unwrap();

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

fn spawn_hello(handle: &Handle) -> SocketAddr {
    use hyper::server::{const_service, service_fn, NewService, Request, Response};
    let addr = "127.0.0.1:0".parse().unwrap();
    let listener = TcpListener::bind(&addr, handle).unwrap();
    let addr = listener.local_addr().unwrap();

    let handle2 = handle.clone();
    let http = hyper::server::Http::<hyper::Chunk>::new();

    let service = const_service(service_fn(|req: Request| {
        req.body()
            .concat2()
            .map(|_| {
                Response::<hyper::Body>::new()
                    .with_header(ContentLength(PHRASE.len() as u64))
                    .with_header(ContentType::plaintext())
                    .with_body(PHRASE)
            })
    }));

    let mut conns = 0;
    handle.spawn(listener.incoming().for_each(move |(socket, _addr)| {
        conns += 1;
        assert_eq!(conns, 1, "should only need 1 connection");
        handle2.spawn(
            http.serve_connection(socket, service.new_service()?)
                .map(|_| ())
                .map_err(|_| ())
        );
        Ok(())
    }).then(|_| Ok(())));
    return addr
}
