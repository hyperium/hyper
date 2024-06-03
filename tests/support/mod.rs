#![allow(dead_code)]
use std::future::Future;
use std::pin::Pin;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};

use hyper::client::HttpConnector;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Client, Request, Response, Server, Version};

pub use futures_util::{
    future, FutureExt as _, StreamExt as _, TryFutureExt as _, TryStreamExt as _,
};
pub use hyper::{ext::Protocol, HeaderMap};
#[allow(unused_imports)]
pub use hyper::{http::Extensions, StatusCode};
pub use std::net::SocketAddr;

#[allow(unused_macros)]
macro_rules! t {
    (
        @impl
        $name:ident,
        parallel: $range:expr,
        $(h2_only: $_h2_only:expr)?
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
        @impl
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
        )*,
        h2_only: $h2_only:expr
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

            if !$h2_only {
                __run_test(__TestConfig {
                    client_version: 1,
                    client_msgs: c.clone(),
                    server_version: 1,
                    server_msgs: s.clone(),
                    parallel: false,
                    connections: 1,
                    proxy: false,
                });
            }

            __run_test(__TestConfig {
                client_version: 2,
                client_msgs: c.clone(),
                server_version: 2,
                server_msgs: s.clone(),
                parallel: false,
                connections: 1,
                proxy: false,
            });

            if !$h2_only {
                __run_test(__TestConfig {
                    client_version: 1,
                    client_msgs: c.clone(),
                    server_version: 1,
                    server_msgs: s.clone(),
                    parallel: false,
                    connections: 1,
                    proxy: true,
                });
            }

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
    (h2_only; $($t:tt)*) => {
        t!(@impl $($t)*, h2_only: true);
    };
    ($($t:tt)*) => {
        t!(@impl $($t)*, h2_only: false);
    };
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
        StatusCode::from_u16($prop_val).expect("status code")
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
    pub protocol: Option<&'static str>,
}

impl Default for __CReq {
    fn default() -> __CReq {
        __CReq {
            method: "GET",
            uri: "/",
            headers: HeaderMap::new(),
            body: Vec::new(),
            protocol: None,
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

    let connector = HttpConnector::new();
    let client = Client::builder()
        .http2_only(cfg.client_version == 2)
        .build::<_, Body>(connector);

    let serve_handles = Arc::new(Mutex::new(cfg.server_msgs));

    let expected_connections = cfg.connections;
    let mut cnt = 0;
    let new_service = make_service_fn(move |_| {
        cnt += 1;
        assert!(
            cnt <= expected_connections,
            "server expected {} connections, received {}",
            expected_connections,
            cnt
        );

        // Move a clone into the service_fn
        let serve_handles = serve_handles.clone();
        future::ok::<_, hyper::Error>(service_fn(move |req: Request<Body>| {
            let (sreq, sres) = serve_handles.lock().unwrap().remove(0);

            assert_eq!(req.uri().path(), sreq.uri, "client path");
            assert_eq!(req.method(), &sreq.method, "client method");
            assert_eq!(req.version(), version, "client version");
            for func in &sreq.headers {
                func(&req.headers());
            }
            let sbody = sreq.body;
            #[allow(deprecated)]
            hyper::body::to_bytes(req).map_ok(move |body| {
                assert_eq!(body.as_ref(), sbody.as_slice(), "client body");

                let mut res = Response::builder()
                    .status(sres.status)
                    .body(Body::from(sres.body))
                    .expect("Response::build");
                *res.headers_mut() = sres.headers;
                res
            })
        }))
    });

    let server = hyper::Server::bind(&SocketAddr::from(([127, 0, 0, 1], 0)))
        .http2_only(cfg.server_version == 2)
        .http2_enable_connect_protocol()
        .serve(new_service);

    let mut addr = server.local_addr();

    tokio::task::spawn(server.map(|result| {
        result.expect("server error");
    }));

    if cfg.proxy {
        let (proxy_addr, proxy) = naive_proxy(ProxyConfig {
            connections: cfg.connections,
            dst: addr,
            version: cfg.server_version,
        });
        tokio::task::spawn(proxy);
        addr = proxy_addr;
    }

    let make_request = Arc::new(
        move |client: &Client<HttpConnector>, creq: __CReq, cres: __CRes| {
            let uri = format!("http://{}{}", addr, creq.uri);
            let mut req = Request::builder()
                .method(creq.method)
                .uri(uri)
                //.headers(creq.headers)
                .body(creq.body.into())
                .expect("Request::build");
            if let Some(protocol) = creq.protocol {
                req.extensions_mut().insert(Protocol::from_static(protocol));
            }
            *req.headers_mut() = creq.headers;
            let cstatus = cres.status;
            let cheaders = cres.headers;
            let cbody = cres.body;

            client
                .request(req)
                .and_then(move |res| {
                    assert_eq!(res.status(), cstatus, "server status");
                    assert_eq!(res.version(), version, "server version");
                    for func in &cheaders {
                        func(&res.headers());
                    }
                    #[allow(deprecated)]
                    hyper::body::to_bytes(res)
                })
                .map_ok(move |body| {
                    assert_eq!(body.as_ref(), cbody.as_slice(), "server body");
                })
                .map(|res| res.expect("client error"))
        },
    );

    let client_futures: Pin<Box<dyn Future<Output = ()> + Send>> = if cfg.parallel {
        let mut client_futures = vec![];
        for (creq, cres) in cfg.client_msgs {
            client_futures.push(make_request(&client, creq, cres));
        }
        drop(client);
        Box::pin(future::join_all(client_futures).map(|_| ()))
    } else {
        let mut client_futures: Pin<Box<dyn Future<Output = Client<HttpConnector>> + Send>> =
            Box::pin(future::ready(client));
        for (creq, cres) in cfg.client_msgs {
            let mk_request = make_request.clone();
            client_futures = Box::pin(client_futures.then(move |client| {
                let fut = mk_request(&client, creq, cres);
                fut.map(move |()| client)
            }));
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

fn naive_proxy(cfg: ProxyConfig) -> (SocketAddr, impl Future<Output = ()>) {
    let client = Client::builder()
        .http2_only(cfg.version == 2)
        .build_http::<Body>();

    let dst_addr = cfg.dst;
    let max_connections = cfg.connections;
    let counter = AtomicUsize::new(0);

    let srv = Server::bind(&([127, 0, 0, 1], 0).into())
        .http2_enable_connect_protocol()
        .serve(make_service_fn(move |_| {
            let prev = counter.fetch_add(1, Ordering::Relaxed);
            assert!(max_connections > prev, "proxy max connections");
            let client = client.clone();
            future::ok::<_, hyper::Error>(service_fn(move |mut req| {
                let uri = format!("http://{}{}", dst_addr, req.uri().path())
                    .parse()
                    .expect("proxy new uri parse");
                *req.uri_mut() = uri;
                client.request(req)
            }))
        }));
    let proxy_addr = srv.local_addr();
    (proxy_addr, srv.map(|res| res.expect("proxy error")))
}
