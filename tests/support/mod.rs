pub extern crate futures;
pub extern crate hyper;
pub extern crate tokio;

pub use std::net::SocketAddr;
pub use self::futures::{future, Future, Stream};
pub use self::futures::sync::oneshot;
pub use self::hyper::{HeaderMap, StatusCode};
pub use self::tokio::runtime::Runtime;

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
                        ..Default::default()
                    },
                    __CRes {
                        ..Default::default()
                    },
                ));
                s.push((
                    __SReq {
                        uri: "/",
                        ..Default::default()
                    },
                    __SRes {
                        ..Default::default()
                    },
                ));
            }

            __run_test(__TestConfig {
                client_version: 2,
                client_msgs: c,
                server_version: 2,
                server_msgs: s,
                parallel: true,
                connections: 1,
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
                    $($c_req_prop: __internal_req_res_prop!($c_req_prop: $c_req_val),)*
                    ..Default::default()
                },
                __CRes {
                    $($c_res_prop: __internal_req_res_prop!($c_res_prop: $c_res_val),)*
                    ..Default::default()
                }
            ),)*];
            let s = vec![$((
                __SReq {
                    $($s_req_prop: __internal_req_res_prop!($s_req_prop: $s_req_val),)*
                    ..Default::default()
                },
                __SRes {
                    $($s_res_prop: __internal_req_res_prop!($s_res_prop: $s_res_val),)*
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
            });

            __run_test(__TestConfig {
                client_version: 2,
                client_msgs: c,
                server_version: 2,
                server_msgs: s,
                parallel: false,
                connections: 1,
            });
        }
    );
}

macro_rules! __internal_req_res_prop {
    (method: $prop_val:expr) => (
        $prop_val
    );
    (status: $prop_val:expr) => (
        StatusCode::from_u16($prop_val).expect("status code")
    );
    (headers: $map:tt) => ({
        #[allow(unused_mut)]
        {
        let mut headers = HeaderMap::new();
        __internal_headers!(headers, $map);
        headers
        }
    });
    ($prop_name:ident: $prop_val:expr) => (
        From::from($prop_val)
    )
}

macro_rules! __internal_headers {
    ($headers:ident, { $($name:expr => $val:expr,)* }) => {
        $(
        $headers.insert($name, $val.to_string().parse().expect("header value"));
        )*
    }
}

#[derive(Clone, Debug, Default)]
pub struct __CReq {
    pub method: &'static str,
    pub uri: &'static str,
    pub headers: HeaderMap,
    pub body: Vec<u8>,
}

#[derive(Clone, Debug, Default)]
pub struct __CRes {
    pub status: hyper::StatusCode,
    pub body: Vec<u8>,
    pub headers: HeaderMap,
}

#[derive(Clone, Debug, Default)]
pub struct __SReq {
    pub method: &'static str,
    pub uri: &'static str,
    pub headers: HeaderMap,
    pub body: Vec<u8>,
}

#[derive(Clone, Debug, Default)]
pub struct __SRes {
    pub status: hyper::StatusCode,
    pub body: Vec<u8>,
    pub headers: HeaderMap,
}

pub struct __TestConfig {
    pub client_version: usize,
    pub client_msgs: Vec<(__CReq, __CRes)>,

    pub server_version: usize,
    pub server_msgs: Vec<(__SReq, __SRes)>,

    pub parallel: bool,
    pub connections: usize,
}

pub fn __run_test(cfg: __TestConfig) {
    extern crate pretty_env_logger;
    use hyper::{Body, Client, Request, Response};
    use hyper::client::HttpConnector;
    use std::sync::{Arc, Mutex};
    let _ = pretty_env_logger::try_init();
    let rt = Runtime::new().expect("new rt");
    let handle = rt.reactor().clone();

    let connector = HttpConnector::new_with_handle(1, handle.clone());
    let client = Client::builder()
        .http2_only(cfg.client_version == 2)
        .executor(rt.executor())
        .build::<_, Body>(connector);

    let serve_handles = Arc::new(Mutex::new(
        cfg.server_msgs
    ));
    let new_service = move || {
        // Move a clone into the service_fn
        let serve_handles = serve_handles.clone();
        hyper::service::service_fn(move |req: Request<Body>| {
            let (sreq, sres) = serve_handles.lock()
                .unwrap()
                .remove(0);

            assert_eq!(req.uri().path(), sreq.uri);
            assert_eq!(req.method(), &sreq.method);
            for (name, value) in &sreq.headers {
                assert_eq!(
                    req.headers()[name],
                    value
                );
            }
            let sbody = sreq.body;
            req.into_body()
                .concat2()
                .map(move |body| {
                    assert_eq!(body.as_ref(), sbody.as_slice());

                    let mut res = Response::builder()
                        .status(sres.status)
                        .body(Body::from(sres.body))
                        .expect("Response::build");
                    *res.headers_mut() = sres.headers;
                    res
                })
        })
    };

    let serve = hyper::server::conn::Http::new()
        .http2_only(cfg.server_version == 2)
        .executor(rt.executor())
        .serve_addr_handle(
            &SocketAddr::from(([127, 0, 0, 1], 0)),
            &handle,
            new_service,
        )
        .expect("serve_addr_handle");

    let addr = serve.incoming_ref().local_addr();
    let exe = rt.executor();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let (success_tx, success_rx) = oneshot::channel();
    let expected_connections = cfg.connections;
    let server = serve
        .fold(0, move |cnt, connecting| {
            let fut = connecting
                .map_err(|never| -> hyper::Error { match never {} })
                .flatten()
                .map_err(|e| panic!("server connection error: {}", e));
            exe.spawn(fut);
            Ok::<_, hyper::Error>(cnt + 1)
        })
        .map(move |cnt| {
            assert_eq!(cnt, expected_connections);
        })
        .map_err(|e| panic!("serve error: {}", e))
        .select2(shutdown_rx)
        .map(move |_| {
            let _ = success_tx.send(());
        })
        .map_err(|_| panic!("shutdown not ok"));

    rt.executor().spawn(server);

    let make_request = Arc::new(move |client: &Client<HttpConnector>, creq: __CReq, cres: __CRes| {
        let uri = format!("http://{}{}", addr, creq.uri);
        let mut req = Request::builder()
            .method(creq.method)
            .uri(uri)
            //.headers(creq.headers)
            .body(creq.body.into())
            .expect("Request::build");
        *req.headers_mut() = creq.headers;
        let cstatus = cres.status;
        let cheaders = cres.headers;
        let cbody = cres.body;

        client.request(req)
            .and_then(move |res| {
                assert_eq!(res.status(), cstatus);
                //assert_eq!(res.version(), c_version);
                for (name, value) in &cheaders {
                    assert_eq!(
                        res.headers()[name],
                        value
                    );
                }
                res.into_body().concat2()
            })
            .map(move |body| {
                assert_eq!(body.as_ref(), cbody.as_slice());
            })
            .map_err(|e| panic!("client error: {}", e))
    });


    let client_futures: Box<Future<Item=(), Error=()> + Send> = if cfg.parallel {
        let mut client_futures = vec![];
        for (creq, cres) in cfg.client_msgs {
            client_futures.push(make_request(&client, creq, cres));
        }
        drop(client);
        Box::new(future::join_all(client_futures).map(|_| ()))
    } else {
        let mut client_futures: Box<Future<Item=Client<HttpConnector>, Error=()> + Send> =
            Box::new(future::ok(client));
        for (creq, cres) in cfg.client_msgs {
            let mk_request = make_request.clone();
            client_futures = Box::new(
                client_futures
                .and_then(move |client| {
                    let fut = mk_request(&client, creq, cres);
                    fut.map(move |()| client)
                })
            );
        }
        Box::new(client_futures.map(|_| ()))
    };

    let client_futures = client_futures.map(move |_| {
        let _ = shutdown_tx.send(());
    });
    rt.executor().spawn(client_futures);
    rt.shutdown_on_idle().wait().expect("rt");
    success_rx
        .map_err(|_| "something panicked")
        .wait()
        .expect("shutdown succeeded");
}

