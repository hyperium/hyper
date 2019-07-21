#![feature(async_await)]
#![feature(test)]
#![deny(warnings)]

extern crate test;

use std::net::SocketAddr;

use futures_util::future::join_all;
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
fn http1_body_both_100kb(b: &mut test::Bencher) {
    let body = &[b'x'; 1024 * 100];
    opts()
        .method(Method::POST)
        .request_body(body)
        .response_body(body)
        .bench(b)
}

#[bench]
fn http1_body_both_10mb(b: &mut test::Bencher) {
    let body = &[b'x'; 1024 * 1024 * 10];
    opts()
        .method(Method::POST)
        .request_body(body)
        .response_body(body)
        .bench(b)
}

#[bench]
fn http1_parallel_x10_empty(b: &mut test::Bencher) {
    opts()
        .parallel(10)
        .bench(b)
}

#[bench]
fn http1_parallel_x10_req_10mb(b: &mut test::Bencher) {
    let body = &[b'x'; 1024 * 1024 * 10];
    opts()
        .parallel(10)
        .method(Method::POST)
        .request_body(body)
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
fn http2_req_100kb(b: &mut test::Bencher) {
    let body = &[b'x'; 1024 * 100];
    opts()
        .http2()
        .method(Method::POST)
        .request_body(body)
        .bench(b)
}

#[bench]
fn http2_parallel_x10_empty(b: &mut test::Bencher) {
    opts()
        .http2()
        .parallel(10)
        .bench(b)
}

#[bench]
fn http2_parallel_x10_req_10mb(b: &mut test::Bencher) {
    let body = &[b'x'; 1024 * 1024 * 10];
    opts()
        .http2()
        .parallel(10)
        .method(Method::POST)
        .request_body(body)
        .http2_stream_window(std::u32::MAX >> 1)
        .http2_conn_window(std::u32::MAX >> 1)
        .bench(b)
}

// ==== Benchmark Options =====

struct Opts {
    http2: bool,
    http2_stream_window: Option<u32>,
    http2_conn_window: Option<u32>,
    parallel_cnt: u32,
    request_method: Method,
    request_body: Option<&'static [u8]>,
    response_body: &'static [u8],
}

fn opts() -> Opts {
    Opts {
        http2: false,
        http2_stream_window: None,
        http2_conn_window: None,
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

    fn http2_stream_window(mut self, sz: impl Into<Option<u32>>) -> Self {
        self.http2_stream_window = sz.into();
        self
    }

    fn http2_conn_window(mut self, sz: impl Into<Option<u32>>) -> Self {
        self.http2_conn_window = sz.into();
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

    fn response_body(mut self, body: &'static [u8]) -> Self {
        self.response_body = body;
        self
    }

    fn parallel(mut self, cnt: u32) -> Self {
        assert!(cnt > 0, "parallel count must be larger than 0");
        self.parallel_cnt = cnt;
        self
    }

    fn bench(self, b: &mut test::Bencher) {
        let _ = pretty_env_logger::try_init();
        // Create a runtime of current thread.
        let mut rt = Runtime::new().unwrap();

        b.bytes = self.response_body.len() as u64 + self.request_body.map(|b| b.len()).unwrap_or(0) as u64;

        let addr = spawn_server(&mut rt, &self);

        let connector = HttpConnector::new(1);
        let client = hyper::Client::builder()
            .http2_only(self.http2)
            .http2_initial_stream_window_size(self.http2_stream_window)
            .http2_initial_connection_window_size(self.http2_conn_window)
            .build::<_, Body>(connector);

        let url: hyper::Uri = format!("http://{}/hello", addr).parse().unwrap();

        let make_request = || {
            let body = self
                .request_body
                .map(Body::from)
                .unwrap_or_else(|| Body::empty());
            let mut req = Request::new(body);
            *req.method_mut() = self.request_method.clone();
            *req.uri_mut() = url.clone();
            req
        };

        let send_request = |req: Request<Body>| {
            let fut = client.request(req);
            async {
                let res = fut.await.expect("client wait");
                let mut body = res.into_body();
                while let Some(_chunk) = body.next().await {}
            }
        };

        if self.parallel_cnt == 1 {
            b.iter(|| {
                let req = make_request();
                rt.block_on(send_request(req));
            });

        } else {
            b.iter(|| {
                let futs = (0..self.parallel_cnt)
                    .map(|_| {
                        let req = make_request();
                        send_request(req)
                    });
                // Await all spawned futures becoming completed.
                rt.block_on(join_all(futs));
            });
        }
    }
}

fn spawn_server(rt: &mut Runtime, opts: &Opts) -> SocketAddr {
    use hyper::service::{make_service_fn, service_fn};
    let addr = "127.0.0.1:0".parse().unwrap();

    let body = opts.response_body;
    let srv = Server::bind(&addr)
        .http2_only(opts.http2)
        .http2_initial_stream_window_size_(opts.http2_stream_window)
        .http2_initial_connection_window_size_(opts.http2_conn_window)
        .serve(make_service_fn( move |_| async move {
            Ok::<_, hyper::Error>(service_fn(move |req: Request<Body>| async move {
                let mut req_body = req.into_body();
                while let Some(_chunk) = req_body.next().await {}
                Ok::<_, hyper::Error>(Response::new(Body::from(body)))
            }))
        }));
    let addr = srv.local_addr();
    rt.spawn(async {
        if let Err(err) = srv.await {
            panic!("server error: {}", err);
        }
    });
    return addr
}
