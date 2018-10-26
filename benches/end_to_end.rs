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

use hyper::{Body, Method, Request, Response, Server};
use hyper::client::HttpConnector;

#[bench]
fn http1_get(b: &mut test::Bencher) {
    opts()
        .bench(b)
}

#[bench]
fn http1_post(b: &mut test::Bencher) {
    opts()
        .method(Method::POST)
        .request_body(b"foo bar baz quux")
        .bench(b)
}

#[bench]
fn http1_get_parallel(b: &mut test::Bencher) {
    opts()
        .parallel(10)
        .bench(b)
}

#[bench]
fn http2_get(b: &mut test::Bencher) {
    opts()
        .http2()
        .bench(b)
}

#[bench]
fn http2_post(b: &mut test::Bencher) {
    opts()
        .http2()
        .method(Method::POST)
        .request_body(b"foo bar baz quux")
        .bench(b)
}

#[bench]
fn http2_get_parallel(b: &mut test::Bencher) {
    opts()
        .http2()
        .parallel(10)
        .bench(b)
}

// ==== Benchmark Options =====

struct Opts {
    http2: bool,
    parallel_cnt: u32,
    request_method: Method,
    request_body: Option<&'static [u8]>,
    response_body: &'static [u8],
}

fn opts() -> Opts {
    Opts {
        http2: false,
        parallel_cnt: 1,
        request_method: Method::GET,
        request_body: None,
        response_body: b"Hello",
    }
}

impl Opts {
    fn http2(mut self) -> Self {
        self.http2 = true;
        self
    }

    fn method(mut self, m: Method) -> Self {
        self.request_method = m;
        self
    }

    fn request_body(mut self, body: &'static [u8]) -> Self {
        self.request_body = Some(body);
        self
    }

    fn parallel(mut self, cnt: u32) -> Self {
        assert!(cnt > 0, "parallel count must be larger than 0");
        self.parallel_cnt = cnt;
        self
    }

    fn bench(self, b: &mut test::Bencher) {
        let _ = pretty_env_logger::try_init();
        let mut rt = Runtime::new().unwrap();
        let addr = spawn_hello(&mut rt, self.response_body);

        let connector = HttpConnector::new(1);
        let client = hyper::Client::builder()
            .http2_only(self.http2)
            .build::<_, Body>(connector);

        let url: hyper::Uri = format!("http://{}/hello", addr).parse().unwrap();

        let make_request = || {
            let body = self
                .request_body
                .map(Body::from)
                .unwrap_or_else(|| Body::empty());
            let mut req = Request::new(body);
            *req.method_mut() = self.request_method.clone();
            req
        };

        if self.parallel_cnt == 1 {
            b.iter(move || {
                let mut req = make_request();
                *req.uri_mut() = url.clone();
                rt.block_on(client.request(req).and_then(|res| {
                    res.into_body().for_each(|_chunk| {
                        Ok(())
                    })
                })).expect("client wait");
            });
        } else {
            b.iter(|| {
                let futs = (0..self.parallel_cnt)
                    .into_iter()
                    .map(|_| {
                        let mut req = make_request();
                        *req.uri_mut() = url.clone();
                        client.request(req).and_then(|res| {
                            res.into_body().for_each(|_chunk| {
                                Ok(())
                            })
                        }).map_err(|e| panic!("client error: {}", e))
                    });
                let _ = rt.block_on(::futures::future::join_all(futs));
            });
        }
    }
}

fn spawn_hello(rt: &mut Runtime, body: &'static [u8]) -> SocketAddr {
    use hyper::service::{service_fn};
    let addr = "127.0.0.1:0".parse().unwrap();

    let srv = Server::bind(&addr)
        .serve(move || {
            service_fn(move |req: Request<Body>| {
                req.into_body()
                    .concat2()
                    .map(move |_| {
                        Response::new(Body::from(body))
                    })
            })
        });
    let addr = srv.local_addr();
    let fut = srv.map_err(|err| panic!("server error: {}", err));
    rt.spawn(fut);
    return addr
}
