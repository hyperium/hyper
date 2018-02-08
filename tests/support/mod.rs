pub extern crate futures;
pub extern crate hyper;
pub extern crate tokio_core;

pub use std::net::SocketAddr;
pub use self::futures::{Future, Stream};
pub use self::hyper::Method::*;
pub use self::hyper::{Headers, StatusCode};

macro_rules! t {
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
            });

            #[cfg(feature = "http2")]
            __run_test(__TestConfig {
                client_version: 2,
                client_msgs: c,
                server_version: 2,
                server_msgs: s,
            });
        }
    );
}

macro_rules! __internal_req_res_prop {
    (method: $prop_val:expr) => (
        $prop_val.parse().expect("method")
    );
    (status: $prop_val:expr) => (
        StatusCode::try_from($prop_val).expect("status code")
    );
    (headers: $map:tt) => ({
        #[allow(unused_mut)]
        let mut headers = Headers::new();
        __internal_headers!(headers, $map);
        headers
    });
    ($prop_name:ident: $prop_val:expr) => (
        From::from($prop_val)
    )
}

macro_rules! __internal_headers {
    ($headers:ident, { $($name:expr => $val:expr,)* }) => {
        $(
        $headers.set_raw($name, $val.to_string());
        )*
    }
}

#[derive(Clone, Debug, Default)]
pub struct __CReq {
    pub method: hyper::Method,
    pub uri: &'static str,
    pub headers: Headers,
    pub body: Vec<u8>,
}

#[derive(Clone, Debug, Default)]
pub struct __CRes {
    pub status: hyper::StatusCode,
    pub body: Vec<u8>,
    pub headers: Headers,
}

#[derive(Clone, Debug, Default)]
pub struct __SReq {
    pub method: hyper::Method,
    pub uri: &'static str,
    pub headers: Headers,
    pub body: Vec<u8>,
}

#[derive(Clone, Debug, Default)]
pub struct __SRes {
    pub status: hyper::StatusCode,
    pub body: Vec<u8>,
    pub headers: Headers,
}

pub struct __TestConfig {
    pub client_version: usize,
    pub client_msgs: Vec<(__CReq, __CRes)>,

    pub server_version: usize,
    pub server_msgs: Vec<(__SReq, __SRes)>,
}

pub fn __run_test(cfg: __TestConfig) {
    extern crate pretty_env_logger;
    use hyper::{Body, Client, Request, Response};
    let _ = pretty_env_logger::try_init();
    let mut core = tokio_core::reactor::Core::new().expect("new core");
    let handle = core.handle();

    #[allow(unused_mut)]
    let mut config = Client::configure();
    #[cfg(feature = "http2")]
    {
        if cfg.client_version == 2 {
            config = config.http2_only();
        }
    }
    let client = config.build(&handle);

    let serve_handles = ::std::sync::Mutex::new(
        cfg.server_msgs
    );
    let service = hyper::server::service_fn(move |req: Request<Body>| -> Box<Future<Item=Response<Body>, Error=hyper::Error>> {
        let (sreq, sres) = serve_handles.lock()
            .unwrap()
            .remove(0);

        assert_eq!(req.uri().path(), sreq.uri);
        assert_eq!(req.method(), &sreq.method);
        for header in sreq.headers.iter() {
            assert_eq!(
                req.headers().get_raw(header.name()).expect(header.name()),
                header.raw()
            );
        }
        let sbody = sreq.body;
        Box::new(req.body()
            .concat2()
            .map(move |body| {
                assert_eq!(body.as_ref(), sbody.as_slice());

                Response::new()
                    .with_status(sres.status)
                    .with_headers(sres.headers)
                    .with_body(sres.body)
            }))
    });
    let new_service = hyper::server::const_service(service);

    #[allow(unused_mut)]
    let mut http = hyper::server::Http::new();
    #[cfg(feature = "http2")]
    {
        if cfg.server_version == 2 {
            http.http2();
        }
    }
    let serve = http.serve_addr_handle2(
            &SocketAddr::from(([127, 0, 0, 1], 0)),
            &handle,
            new_service,
        )
        .expect("serve_addr_handle");

    let addr = serve.incoming_ref().local_addr();
    let handle2 = handle.clone();
    handle.spawn(serve.for_each(move |conn: hyper::server::Connection2<_, _>| {
        handle2.spawn(conn.map(|_| ()).map_err(|e| panic!("server connection error: {}", e)));
        Ok(())
    }).map_err(|e| panic!("serve error: {}", e)));

    for (creq, cres) in cfg.client_msgs {
        let uri = format!("http://{}{}", addr, creq.uri).parse().expect("uri parse");
        let mut req = Request::new(creq.method, uri);
        *req.headers_mut() = creq.headers;
        if !creq.body.is_empty() {
            req.set_body(creq.body);
        }
        let cstatus = cres.status;
        let cheaders = cres.headers;
        let cbody = cres.body;
        let fut = client.request(req)
            .and_then(move |res| {
                assert_eq!(res.status(), cstatus);
                //assert_eq!(res.version(), c_version);
                for header in cheaders.iter() {
                    assert_eq!(
                        res.headers().get_raw(header.name()).expect(header.name()),
                        header.raw()
                    );
                }
                res.body().concat2()
            })
            .and_then(move |body| {
                assert_eq!(body.as_ref(), cbody.as_slice());
                Ok(())
            });
        core.run(fut).expect("core.run client");
    }
}
