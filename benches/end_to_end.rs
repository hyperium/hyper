#![feature(test)]
#![deny(warnings)]

extern crate test;
mod support;

// TODO: Reimplement parallel for HTTP/1

use std::convert::Infallible;
use std::net::SocketAddr;

use futures_util::future::join_all;

use http_body_util::BodyExt;
use hyper::{Method, Request, Response};

type BoxedBody = http_body_util::combinators::BoxBody<bytes::Bytes, Infallible>;

// HTTP1

#[bench]
fn http1_consecutive_x1_empty(b: &mut test::Bencher) {
    opts().bench(b)
}

#[bench]
fn http1_consecutive_x1_req_10b(b: &mut test::Bencher) {
    opts()
        .method(Method::POST)
        .request_body(&[b's'; 10])
        .bench(b)
}

#[bench]
fn http1_consecutive_x1_both_100kb(b: &mut test::Bencher) {
    let body = &[b'x'; 1024 * 100];
    opts()
        .method(Method::POST)
        .request_body(body)
        .response_body(body)
        .bench(b)
}

#[bench]
fn http1_consecutive_x1_both_10mb(b: &mut test::Bencher) {
    let body = &[b'x'; 1024 * 1024 * 10];
    opts()
        .method(Method::POST)
        .request_body(body)
        .response_body(body)
        .bench(b)
}

#[bench]
#[ignore]
fn http1_parallel_x10_empty(b: &mut test::Bencher) {
    opts().parallel(10).bench(b)
}

#[bench]
#[ignore]
fn http1_parallel_x10_req_10mb(b: &mut test::Bencher) {
    let body = &[b'x'; 1024 * 1024 * 10];
    opts()
        .parallel(10)
        .method(Method::POST)
        .request_body(body)
        .bench(b)
}

#[bench]
#[ignore]
fn http1_parallel_x10_req_10kb_100_chunks(b: &mut test::Bencher) {
    let body = &[b'x'; 1024 * 10];
    opts()
        .parallel(10)
        .method(Method::POST)
        .request_chunks(body, 100)
        .bench(b)
}

#[bench]
#[ignore]
fn http1_parallel_x10_res_1mb(b: &mut test::Bencher) {
    let body = &[b'x'; 1024 * 1024 * 1];
    opts().parallel(10).response_body(body).bench(b)
}

#[bench]
#[ignore]
fn http1_parallel_x10_res_10mb(b: &mut test::Bencher) {
    let body = &[b'x'; 1024 * 1024 * 10];
    opts().parallel(10).response_body(body).bench(b)
}

// HTTP2

const HTTP2_MAX_WINDOW: u32 = std::u32::MAX >> 1;

#[bench]
fn http2_consecutive_x1_empty(b: &mut test::Bencher) {
    opts().http2().bench(b)
}

#[bench]
fn http2_consecutive_x1_req_10b(b: &mut test::Bencher) {
    opts()
        .http2()
        .method(Method::POST)
        .request_body(&[b's'; 10])
        .bench(b)
}

#[bench]
fn http2_consecutive_x1_req_100kb(b: &mut test::Bencher) {
    let body = &[b'x'; 1024 * 100];
    opts()
        .http2()
        .method(Method::POST)
        .request_body(body)
        .bench(b)
}

#[bench]
fn http2_parallel_x10_empty(b: &mut test::Bencher) {
    opts().http2().parallel(10).bench(b)
}

#[bench]
fn http2_parallel_x10_req_10mb(b: &mut test::Bencher) {
    let body = &[b'x'; 1024 * 1024 * 10];
    opts()
        .http2()
        .parallel(10)
        .method(Method::POST)
        .request_body(body)
        .http2_stream_window(HTTP2_MAX_WINDOW)
        .http2_conn_window(HTTP2_MAX_WINDOW)
        .bench(b)
}

#[bench]
#[ignore]
fn http2_parallel_x10_req_10kb_100_chunks(b: &mut test::Bencher) {
    let body = &[b'x'; 1024 * 10];
    opts()
        .http2()
        .parallel(10)
        .method(Method::POST)
        .request_chunks(body, 100)
        .bench(b)
}

#[bench]
#[ignore]
fn http2_parallel_x10_req_10kb_100_chunks_adaptive_window(b: &mut test::Bencher) {
    let body = &[b'x'; 1024 * 10];
    opts()
        .http2()
        .parallel(10)
        .method(Method::POST)
        .request_chunks(body, 100)
        .http2_adaptive_window()
        .bench(b)
}

#[bench]
#[ignore]
fn http2_parallel_x10_req_10kb_100_chunks_max_window(b: &mut test::Bencher) {
    let body = &[b'x'; 1024 * 10];
    opts()
        .http2()
        .parallel(10)
        .method(Method::POST)
        .request_chunks(body, 100)
        .http2_stream_window(HTTP2_MAX_WINDOW)
        .http2_conn_window(HTTP2_MAX_WINDOW)
        .bench(b)
}

#[bench]
fn http2_parallel_x10_res_1mb(b: &mut test::Bencher) {
    let body = &[b'x'; 1024 * 1024 * 1];
    opts()
        .http2()
        .parallel(10)
        .response_body(body)
        .http2_stream_window(HTTP2_MAX_WINDOW)
        .http2_conn_window(HTTP2_MAX_WINDOW)
        .bench(b)
}

#[bench]
fn http2_parallel_x10_res_10mb(b: &mut test::Bencher) {
    let body = &[b'x'; 1024 * 1024 * 10];
    opts()
        .http2()
        .parallel(10)
        .response_body(body)
        .http2_stream_window(HTTP2_MAX_WINDOW)
        .http2_conn_window(HTTP2_MAX_WINDOW)
        .bench(b)
}

// ==== Benchmark Options =====

#[derive(Clone)]
struct Opts {
    http2: bool,
    http2_stream_window: Option<u32>,
    http2_conn_window: Option<u32>,
    http2_adaptive_window: bool,
    parallel_cnt: u32,
    request_method: Method,
    request_body: Option<&'static [u8]>,
    request_chunks: usize,
    response_body: &'static [u8],
}

fn opts() -> Opts {
    Opts {
        http2: false,
        http2_stream_window: None,
        http2_conn_window: None,
        http2_adaptive_window: false,
        parallel_cnt: 1,
        request_method: Method::GET,
        request_body: None,
        request_chunks: 0,
        response_body: b"",
    }
}

impl Opts {
    fn http2(mut self) -> Self {
        self.http2 = true;
        self
    }

    fn http2_stream_window(mut self, sz: impl Into<Option<u32>>) -> Self {
        assert!(!self.http2_adaptive_window);
        self.http2_stream_window = sz.into();
        self
    }

    fn http2_conn_window(mut self, sz: impl Into<Option<u32>>) -> Self {
        assert!(!self.http2_adaptive_window);
        self.http2_conn_window = sz.into();
        self
    }

    fn http2_adaptive_window(mut self) -> Self {
        assert!(self.http2_stream_window.is_none());
        assert!(self.http2_conn_window.is_none());
        self.http2_adaptive_window = true;
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

    fn request_chunks(mut self, chunk: &'static [u8], cnt: usize) -> Self {
        assert!(cnt > 0);
        self.request_body = Some(chunk);
        self.request_chunks = cnt;
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
        use std::sync::Arc;
        let _ = pretty_env_logger::try_init();
        // Create a runtime of current thread.
        let rt = Arc::new(
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("rt build"),
        );
        //let exec = rt.clone();

        let req_len = self.request_body.map(|b| b.len()).unwrap_or(0) as u64;
        let req_len = if self.request_chunks > 0 {
            req_len * self.request_chunks as u64
        } else {
            req_len
        };
        let bytes_per_iter = (req_len + self.response_body.len() as u64) * self.parallel_cnt as u64;
        b.bytes = bytes_per_iter;

        let addr = spawn_server(&rt, &self);

        enum Client {
            Http1(hyper::client::conn::http1::SendRequest<BoxedBody>),
            Http2(hyper::client::conn::http2::SendRequest<BoxedBody>),
        }

        let mut client = rt.block_on(async {
            if self.http2 {
                let tcp = tokio::net::TcpStream::connect(&addr).await.unwrap();
                let io = support::TokioIo::new(tcp);
                let (tx, conn) = hyper::client::conn::http2::Builder::new(support::TokioExecutor)
                    .initial_stream_window_size(self.http2_stream_window)
                    .initial_connection_window_size(self.http2_conn_window)
                    .adaptive_window(self.http2_adaptive_window)
                    .handshake(io)
                    .await
                    .unwrap();
                tokio::spawn(conn);
                Client::Http2(tx)
            } else if self.parallel_cnt > 1 {
                todo!("http/1 parallel >1");
            } else {
                let tcp = tokio::net::TcpStream::connect(&addr).await.unwrap();
                let io = support::TokioIo::new(tcp);
                let (tx, conn) = hyper::client::conn::http1::Builder::new()
                    .handshake(io)
                    .await
                    .unwrap();
                tokio::spawn(conn);
                Client::Http1(tx)
            }
        });

        let url: hyper::Uri = format!("http://{}/hello", addr).parse().unwrap();

        let make_request = || {
            let chunk_cnt = self.request_chunks;
            let body = if chunk_cnt > 0 {
                /*
                let (mut tx, body) = Body::channel();
                let chunk = self
                    .request_body
                    .expect("request_chunks means request_body");
                exec.spawn(async move {
                    for _ in 0..chunk_cnt {
                        tx.send_data(chunk.into()).await.expect("send_data");
                    }
                });
                body
                */
                todo!("request_chunks");
            } else if let Some(chunk) = self.request_body {
                http_body_util::Full::new(bytes::Bytes::from(chunk)).boxed()
            } else {
                http_body_util::Empty::new().boxed()
            };
            let mut req = Request::new(body);
            *req.method_mut() = self.request_method.clone();
            *req.uri_mut() = url.clone();
            req
        };

        let mut send_request = |req| {
            let fut = match client {
                Client::Http1(ref mut tx) => {
                    futures_util::future::Either::Left(tx.send_request(req))
                }
                Client::Http2(ref mut tx) => {
                    futures_util::future::Either::Right(tx.send_request(req))
                }
            };
            async {
                let res = fut.await.expect("client wait");
                let mut body = res.into_body();
                while let Some(_chunk) = body.frame().await {}
            }
        };

        if self.parallel_cnt == 1 {
            b.iter(|| {
                let req = make_request();
                rt.block_on(send_request(req));
            });
        } else {
            b.iter(|| {
                let futs = (0..self.parallel_cnt).map(|_| {
                    let req = make_request();
                    send_request(req)
                });
                // Await all spawned futures becoming completed.
                rt.block_on(join_all(futs));
            });
        }
    }
}

fn spawn_server(rt: &tokio::runtime::Runtime, opts: &Opts) -> SocketAddr {
    use http_body_util::Full;
    use hyper::service::service_fn;
    use tokio::net::TcpListener;
    let addr = "127.0.0.1:0".parse::<std::net::SocketAddr>().unwrap();

    let listener = rt.block_on(async { TcpListener::bind(&addr).await.unwrap() });

    let addr = listener.local_addr().unwrap();
    let body = opts.response_body;
    let opts = opts.clone();
    rt.spawn(async move {
        while let Ok((sock, _)) = listener.accept().await {
            let io = support::TokioIo::new(sock);
            if opts.http2 {
                tokio::spawn(
                    hyper::server::conn::http2::Builder::new(support::TokioExecutor)
                        .initial_stream_window_size(opts.http2_stream_window)
                        .initial_connection_window_size(opts.http2_conn_window)
                        .adaptive_window(opts.http2_adaptive_window)
                        .serve_connection(
                            io,
                            service_fn(move |req: Request<hyper::body::Incoming>| async move {
                                let mut req_body = req.into_body();
                                while let Some(_chunk) = req_body.frame().await {}
                                Ok::<_, std::convert::Infallible>(Response::new(
                                    Full::<bytes::Bytes>::from(body),
                                ))
                            }),
                        ),
                );
            } else {
                tokio::spawn(hyper::server::conn::http1::Builder::new().serve_connection(
                    io,
                    service_fn(move |req: Request<hyper::body::Incoming>| async move {
                        let mut req_body = req.into_body();
                        while let Some(_chunk) = req_body.frame().await {}
                        Ok::<_, std::convert::Infallible>(Response::new(
                            Full::<bytes::Bytes>::from(body),
                        ))
                    }),
                ));
            }
        }
    });
    addr
}
