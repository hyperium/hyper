#![allow(dead_code)]
use std::future::Future;
use std::pin::Pin;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};

use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::server;
use tokio::net::{TcpListener, TcpStream};

use hyper::service::service_fn;
use hyper::{body::Incoming as IncomingBody, Request, Response, Version};

pub use futures_util::{
    future, FutureExt as _, StreamExt as _, TryFutureExt as _, TryStreamExt as _,
};
pub use hyper::HeaderMap;
pub use std::net::SocketAddr;

mod tokiort;
#[allow(unused)]
pub use tokiort::{TokioExecutor, TokioIo, TokioTimer};

#[allow(unused_macros)]
macro_rules! t {
    (
        $name:ident,
        parallel: $range:expr
    ) => (
        #[test]
        fn $name() {

            let mut c = vec![];
            let mut s = vec![];

            for _i in $range {
                c.push((
                    __CReq {
                        uri: "/",
                        body: vec![b'x'; 8192],
                        ..Default::default()
                    },
                    __CRes {
                        body: vec![b'x'; 8192],
                        ..Default::default()
                    },
                ));
                s.push((
                    __SReq {
                        uri: "/",
                        body: vec![b'x'; 8192],
                        ..Default::default()
                    },
                    __SRes {
                        body: vec![b'x'; 8192],
                        ..Default::default()
                    },
                ));
            }

            __run_test(__TestConfig {
                client_version: 2,
                client_msgs: c.clone(),
                server_version: 2,
                server_msgs: s.clone(),
                parallel: true,
                connections: 1,
                proxy: false,
            });

            __run_test(__TestConfig {
                client_version: 2,
                client_msgs: c,
                server_version: 2,
                server_msgs: s,
                parallel: true,
                connections: 1,
                proxy: true,
            });
        }
    );
    (
        $name:ident,
        client: $(
            request: $(
                $c_req_prop:ident: $c_req_val:tt,
            )*;
            response: $(
                $c_res_prop:ident: $c_res_val:tt,
            )*;
        )*
        server: $(
            request: $(
                $s_req_prop:ident: $s_req_val:tt,
            )*;
            response: $(
                $s_res_prop:ident: $s_res_val:tt,
            )*;
        )*
    ) => (
        #[test]
        fn $name() {
            let c = vec![$((
                __CReq {
                    $($c_req_prop: __internal_map_prop!($c_req_prop: $c_req_val),)*
                    ..Default::default()
                },
                __CRes {
                    $($c_res_prop: __internal_eq_prop!($c_res_prop: $c_res_val),)*
                    ..Default::default()
                }
            ),)*];
            let s = vec![$((
                __SReq {
                    $($s_req_prop: __internal_eq_prop!($s_req_prop: $s_req_val),)*
                    ..Default::default()
                },
                __SRes {
                    $($s_res_prop: __internal_map_prop!($s_res_prop: $s_res_val),)*
                    ..Default::default()
                }
            ),)*];

            __run_test(__TestConfig {
                client_version: 1,
                client_msgs: c.clone(),
                server_version: 1,
                server_msgs: s.clone(),
                parallel: false,
                connections: 1,
                proxy: false,
            });

            __run_test(__TestConfig {
                client_version: 2,
                client_msgs: c.clone(),
                server_version: 2,
                server_msgs: s.clone(),
                parallel: false,
                connections: 1,
                proxy: false,
            });

            __run_test(__TestConfig {
                client_version: 1,
                client_msgs: c.clone(),
                server_version: 1,
                server_msgs: s.clone(),
                parallel: false,
                connections: 1,
                proxy: true,
            });

            __run_test(__TestConfig {
                client_version: 2,
                client_msgs: c,
                server_version: 2,
                server_msgs: s,
                parallel: false,
                connections: 1,
                proxy: true,
            });
        }
    );
}

macro_rules! __internal_map_prop {
    (headers: $map:tt) => {{
        #[allow(unused_mut)]
        {
            let mut headers = HeaderMap::new();
            __internal_headers_map!(headers, $map);
            headers
        }
    }};
    ($name:tt: $val:tt) => {{
        __internal_req_res_prop!($name: $val)
    }};
}

macro_rules! __internal_eq_prop {
    (headers: $map:tt) => {{
        #[allow(unused_mut)]
        {
            let mut headers = Vec::<std::sync::Arc<dyn Fn(&hyper::HeaderMap) + Send + Sync>>::new();
            __internal_headers_eq!(headers, $map);
            headers
        }
    }};
    ($name:tt: $val:tt) => {{
        __internal_req_res_prop!($name: $val)
    }};
}

macro_rules! __internal_req_res_prop {
    (method: $prop_val:expr) => {
        $prop_val
    };
    (status: $prop_val:expr) => {
        hyper::StatusCode::from_u16($prop_val).expect("status code")
    };
    ($prop_name:ident: $prop_val:expr) => {
        From::from($prop_val)
    };
}

macro_rules! __internal_headers_map {
    ($headers:ident, { $($name:expr => $val:expr,)* }) => {
        $(
        $headers.insert($name, $val.to_string().parse().expect("header value"));
        )*
    }
}

macro_rules! __internal_headers_eq {
    (@pat $name: expr, $pat:pat) => {
        std::sync::Arc::new(move |__hdrs: &hyper::HeaderMap| {
            match __hdrs.get($name) {
                $pat => (),
                other => panic!("headers[{}] was not {}: {:?}", stringify!($name), stringify!($pat), other),
            }
        }) as std::sync::Arc<dyn Fn(&hyper::HeaderMap) + Send + Sync>
    };
    (@val $name: expr, NONE) => {{
        __internal_headers_eq!(@pat $name, None);
    }};
    (@val $name: expr, SOME) => {{
        __internal_headers_eq!(@pat $name, Some(_))
    }};
    (@val $name: expr, $val:expr) => ({
        let __val = Option::from($val);
        std::sync::Arc::new(move |__hdrs: &hyper::HeaderMap| {
            if let Some(ref val) = __val {
                assert_eq!(__hdrs.get($name).expect(stringify!($name)), val.to_string().as_str(), stringify!($name));
            } else {
                assert_eq!(__hdrs.get($name), None, stringify!($name));
            }
        }) as std::sync::Arc<dyn Fn(&hyper::HeaderMap) + Send + Sync>
    });
    ($headers:ident, { $($name:expr => $val:tt,)* }) => {{
        $(
        $headers.push(__internal_headers_eq!(@val $name, $val));
        )*
    }}
}

#[derive(Clone, Debug)]
pub struct __CReq {
    pub method: &'static str,
    pub uri: &'static str,
    pub headers: HeaderMap,
    pub body: Vec<u8>,
}

impl Default for __CReq {
    fn default() -> __CReq {
        __CReq {
            method: "GET",
            uri: "/",
            headers: HeaderMap::new(),
            body: Vec::new(),
        }
    }
}

#[derive(Clone, Default)]
pub struct __CRes {
    pub status: hyper::StatusCode,
    pub body: Vec<u8>,
    pub headers: __HeadersEq,
}

#[derive(Clone)]
pub struct __SReq {
    pub method: &'static str,
    pub uri: &'static str,
    pub headers: __HeadersEq,
    pub body: Vec<u8>,
}

impl Default for __SReq {
    fn default() -> __SReq {
        __SReq {
            method: "GET",
            uri: "/",
            headers: Vec::new(),
            body: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct __SRes {
    pub status: hyper::StatusCode,
    pub body: Vec<u8>,
    pub headers: HeaderMap,
}

pub type __HeadersEq = Vec<Arc<dyn Fn(&HeaderMap) + Send + Sync>>;

pub struct __TestConfig {
    pub client_version: usize,
    pub client_msgs: Vec<(__CReq, __CRes)>,

    pub server_version: usize,
    pub server_msgs: Vec<(__SReq, __SRes)>,

    pub parallel: bool,
    pub connections: usize,
    pub proxy: bool,
}

pub fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("new rt")
}

pub fn __run_test(cfg: __TestConfig) {
    let _ = pretty_env_logger::try_init();
    runtime().block_on(async_test(cfg));
}

async fn async_test(cfg: __TestConfig) {
    assert_eq!(cfg.client_version, cfg.server_version);

    let version = if cfg.client_version == 2 {
        Version::HTTP_2
    } else {
        Version::HTTP_11
    };

    let http2_only = cfg.server_version == 2;

    let serve_handles = Arc::new(Mutex::new(cfg.server_msgs));

    let listener = TcpListener::bind(&SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .unwrap();

    let mut addr = listener.local_addr().unwrap();

    let expected_connections = cfg.connections;
    tokio::task::spawn(async move {
        let mut cnt = 0;

        cnt += 1;
        assert!(
            cnt <= expected_connections,
            "server expected {} connections, received {}",
            expected_connections,
            cnt
        );

        loop {
            let (stream, _) = listener.accept().await.expect("server error");
            let io = TokioIo::new(stream);

            // Move a clone into the service_fn
            let serve_handles = serve_handles.clone();
            let service = service_fn(move |req: Request<IncomingBody>| {
                let (sreq, sres) = serve_handles.lock().unwrap().remove(0);

                assert_eq!(req.uri().path(), sreq.uri, "client path");
                assert_eq!(req.method(), &sreq.method, "client method");
                assert_eq!(req.version(), version, "client version");
                for func in &sreq.headers {
                    func(&req.headers());
                }
                let sbody = sreq.body;
                req.collect().map_ok(move |collected| {
                    let body = collected.to_bytes();
                    assert_eq!(body.as_ref(), sbody.as_slice(), "client body");

                    let mut res = Response::builder()
                        .status(sres.status)
                        .body(Full::new(Bytes::from(sres.body)))
                        .expect("Response::build");
                    *res.headers_mut() = sres.headers;
                    res
                })
            });

            tokio::task::spawn(async move {
                if http2_only {
                    server::conn::http2::Builder::new(TokioExecutor)
                        .serve_connection(io, service)
                        .await
                        .expect("server error");
                } else {
                    server::conn::http1::Builder::new()
                        .serve_connection(io, service)
                        .await
                        .expect("server error");
                }
            });
        }
    });

    if cfg.proxy {
        let (proxy_addr, proxy) = naive_proxy(ProxyConfig {
            connections: cfg.connections,
            dst: addr,
            version: cfg.server_version,
        })
        .await;
        tokio::task::spawn(proxy);
        addr = proxy_addr;
    }

    let make_request = Arc::new(move |creq: __CReq, cres: __CRes| {
        let uri = format!("http://{}{}", addr, creq.uri);
        let mut req = Request::builder()
            .method(creq.method)
            .uri(uri)
            //.headers(creq.headers)
            .body(Full::new(Bytes::from(creq.body)))
            .expect("Request::build");
        *req.headers_mut() = creq.headers;
        let cstatus = cres.status;
        let cheaders = cres.headers;
        let cbody = cres.body;

        async move {
            let stream = TcpStream::connect(addr).await.unwrap();
            let io = TokioIo::new(stream);

            let res = if http2_only {
                let (mut sender, conn) = hyper::client::conn::http2::Builder::new(TokioExecutor)
                    .handshake(io)
                    .await
                    .unwrap();

                tokio::task::spawn(async move {
                    if let Err(err) = conn.await {
                        panic!("{:?}", err);
                    }
                });
                sender.send_request(req).await.unwrap()
            } else {
                let (mut sender, conn) = hyper::client::conn::http1::Builder::new()
                    .handshake(io)
                    .await
                    .unwrap();

                tokio::task::spawn(async move {
                    if let Err(err) = conn.await {
                        panic!("{:?}", err);
                    }
                });
                sender.send_request(req).await.unwrap()
            };

            assert_eq!(res.status(), cstatus, "server status");
            assert_eq!(res.version(), version, "server version");
            for func in &cheaders {
                func(&res.headers());
            }

            let body = res.collect().await.unwrap().to_bytes();

            assert_eq!(body.as_ref(), cbody.as_slice(), "server body");
        }
    });

    let client_futures: Pin<Box<dyn Future<Output = ()> + Send>> = if cfg.parallel {
        let mut client_futures = vec![];
        for (creq, cres) in cfg.client_msgs {
            client_futures.push(make_request(creq, cres));
        }
        Box::pin(future::join_all(client_futures).map(|_| ()))
    } else {
        let mut client_futures: Pin<Box<dyn Future<Output = ()> + Send>> =
            Box::pin(future::ready(()));
        for (creq, cres) in cfg.client_msgs {
            let mk_request = make_request.clone();
            client_futures = Box::pin(client_futures.then(move |_| mk_request(creq, cres)));
        }
        Box::pin(client_futures.map(|_| ()))
    };

    client_futures.await;
}

struct ProxyConfig {
    connections: usize,
    dst: SocketAddr,
    version: usize,
}

async fn naive_proxy(cfg: ProxyConfig) -> (SocketAddr, impl Future<Output = ()>) {
    let dst_addr = cfg.dst;
    let max_connections = cfg.connections;
    let counter = AtomicUsize::new(0);
    let http2_only = cfg.version == 2;

    let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .unwrap();

    let proxy_addr = listener.local_addr().unwrap();

    let fut = async move {
        tokio::task::spawn(async move {
            let prev = counter.fetch_add(1, Ordering::Relaxed);
            assert!(max_connections > prev, "proxy max connections");

            loop {
                let (stream, _) = listener.accept().await.unwrap();
                let io = TokioIo::new(stream);

                let service = service_fn(move |mut req| {
                    async move {
                        let uri = format!("http://{}{}", dst_addr, req.uri().path())
                            .parse()
                            .expect("proxy new uri parse");
                        *req.uri_mut() = uri;

                        // Make the client request
                        let uri = req.uri().host().expect("uri has no host");
                        let port = req.uri().port_u16().expect("uri has no port");

                        let stream = TcpStream::connect(format!("{}:{}", uri, port))
                            .await
                            .unwrap();
                        let io = TokioIo::new(stream);

                        let resp = if http2_only {
                            let (mut sender, conn) =
                                hyper::client::conn::http2::Builder::new(TokioExecutor)
                                    .handshake(io)
                                    .await
                                    .unwrap();

                            tokio::task::spawn(async move {
                                if let Err(err) = conn.await {
                                    panic!("{:?}", err);
                                }
                            });

                            sender.send_request(req).await?
                        } else {
                            let builder = hyper::client::conn::http1::Builder::new();
                            let (mut sender, conn) = builder.handshake(io).await.unwrap();

                            tokio::task::spawn(async move {
                                if let Err(err) = conn.await {
                                    panic!("{:?}", err);
                                }
                            });

                            sender.send_request(req).await?
                        };

                        let (mut parts, body) = resp.into_parts();

                        // Remove the Connection header for HTTP/1.1 proxy connections.
                        if !http2_only {
                            parts.headers.remove("Connection");
                        }

                        let mut builder = Response::builder().status(parts.status);
                        *builder.headers_mut().unwrap() = parts.headers;

                        Result::<Response<hyper::body::Incoming>, hyper::Error>::Ok(
                            builder.body(body).unwrap(),
                        )
                    }
                });

                if http2_only {
                    server::conn::http2::Builder::new(TokioExecutor)
                        .serve_connection(io, service)
                        .await
                        .unwrap();
                } else {
                    server::conn::http1::Builder::new()
                        .serve_connection(io, service)
                        .await
                        .unwrap();
                }
            }
        });
    };

    (proxy_addr, fut)
}
