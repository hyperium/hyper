#![deny(warnings)]
extern crate bytes;
extern crate hyper;
extern crate futures;
extern crate tokio_core;
extern crate tokio_io;
extern crate pretty_env_logger;

use std::io::{self, Read, Write};
use std::net::TcpListener;
use std::thread;
use std::time::Duration;

use hyper::{Body, Client, Method, Request, StatusCode};

use futures::{Future, Stream};
use futures::sync::oneshot;

use tokio_core::reactor::{Core, Handle};

fn s(buf: &[u8]) -> &str {
    ::std::str::from_utf8(buf).unwrap()
}

macro_rules! test {
    (
        name: $name:ident,
        server:
            expected: $server_expected:expr,
            reply: $server_reply:expr,
        client:
            request:
                method: $client_method:ident,
                url: $client_url:expr,
                headers: { $($request_header_name:expr => $request_header_val:expr,)* },
                body: $request_body:expr,

            response:
                status: $client_status:ident,
                headers: { $($response_header_name:expr => $response_header_val:expr,)* },
                body: $response_body:expr,
    ) => (
        test! {
            name: $name,
            server:
                expected: $server_expected,
                reply: $server_reply,
            client:
                set_host: true,
                request:
                    method: $client_method,
                    url: $client_url,
                    headers: { $($request_header_name => $request_header_val,)* },
                    body: $request_body,

                response:
                    status: $client_status,
                    headers: { $($response_header_name => $response_header_val,)* },
                    body: $response_body,
        }
    );
    (
        name: $name:ident,
        server:
            expected: $server_expected:expr,
            reply: $server_reply:expr,
        client:
            set_host: $set_host:expr,
            request:
                method: $client_method:ident,
                url: $client_url:expr,
                headers: { $($request_header_name:expr => $request_header_val:expr,)* },
                body: $request_body:expr,

            response:
                status: $client_status:ident,
                headers: { $($response_header_name:expr => $response_header_val:expr,)* },
                body: $response_body:expr,
    ) => (
        #[test]
        fn $name() {
            let _ = pretty_env_logger::try_init();
            let mut core = Core::new().unwrap();

            let res = test! {
                INNER;
                name: $name,
                core: &mut core,
                server:
                    expected: $server_expected,
                    reply: $server_reply,
                client:
                    set_host: $set_host,
                    request:
                        method: $client_method,
                        url: $client_url,
                        headers: { $($request_header_name => $request_header_val,)* },
                        body: $request_body,
            }.unwrap();


            assert_eq!(res.status(), StatusCode::$client_status);
            $(
                assert_eq!(res.headers()[$response_header_name], $response_header_val);
            )*

            let body = core.run(res.into_body().into_stream().concat2()).unwrap();

            let expected_res_body = Option::<&[u8]>::from($response_body)
                .unwrap_or_default();
            assert_eq!(body.as_ref(), expected_res_body);
        }
    );
    (
        name: $name:ident,
        server:
            expected: $server_expected:expr,
            reply: $server_reply:expr,
        client:
            request:
                method: $client_method:ident,
                url: $client_url:expr,
                headers: { $($request_header_name:expr => $request_header_val:expr,)* },
                body: $request_body:expr,

            error: $err:expr,
    ) => (
        #[test]
        fn $name() {
            let _ = pretty_env_logger::try_init();
            let mut core = Core::new().unwrap();

            let err = test! {
                INNER;
                name: $name,
                core: &mut core,
                server:
                    expected: $server_expected,
                    reply: $server_reply,
                client:
                    set_host: true,
                    request:
                        method: $client_method,
                        url: $client_url,
                        headers: { $($request_header_name => $request_header_val,)* },
                        body: $request_body,
            }.unwrap_err();
            if !$err(&err) {
                panic!("unexpected error: {:?}", err)
            }
        }
    );

    (
        INNER;
        name: $name:ident,
        core: $core:expr,
        server:
            expected: $server_expected:expr,
            reply: $server_reply:expr,
        client:
            set_host: $set_host:expr,
            request:
                method: $client_method:ident,
                url: $client_url:expr,
                headers: { $($request_header_name:expr => $request_header_val:expr,)* },
                body: $request_body:expr,
    ) => ({
        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = server.local_addr().unwrap();
        let core = $core;

        let mut config = Client::configure();
        if !$set_host {
            config = config.set_host(false);
        }
        let client = config.build(&core.handle());

        let body = if let Some(body) = $request_body {
            let body: &'static str = body;
            body.into()
        } else {
            Body::empty()
        };
        let req = Request::builder()
            .method(Method::$client_method)
            .uri(&*format!($client_url, addr=addr))
        $(
            .header($request_header_name, $request_header_val)
        )*
            .body(body)
            .unwrap();

        let res = client.request(req);

        let (tx, rx) = oneshot::channel();

        let thread = thread::Builder::new()
            .name(format!("tcp-server<{}>", stringify!($name)));
        thread.spawn(move || {
            let mut inc = server.accept().unwrap().0;
            inc.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            inc.set_write_timeout(Some(Duration::from_secs(5))).unwrap();
            let expected = format!($server_expected, addr=addr);
            let mut buf = [0; 4096];
            let mut n = 0;
            while n < buf.len() && n < expected.len() {
                n += match inc.read(&mut buf[n..]) {
                    Ok(n) => n,
                    Err(e) => panic!("failed to read request, partially read = {:?}, error: {}", s(&buf[..n]), e),
                };
            }
            assert_eq!(s(&buf[..n]), expected);

            inc.write_all($server_reply.as_ref()).unwrap();
            let _ = tx.send(());
        }).unwrap();

        let rx = rx.map_err(|_| hyper::Error::Io(io::Error::new(io::ErrorKind::Other, "thread panicked")));

        let work = res.join(rx).map(|r| r.0);

        core.run(work)
    });
}

static REPLY_OK: &'static str = "HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n";

test! {
    name: client_get,

    server:
        expected: "GET / HTTP/1.1\r\nhost: {addr}\r\n\r\n",
        reply: REPLY_OK,

    client:
        request:
            method: GET,
            url: "http://{addr}/",
            headers: {},
            body: None,
        response:
            status: OK,
            headers: {
                "Content-Length" => "0",
            },
            body: None,
}

test! {
    name: client_get_query,

    server:
        expected: "GET /foo?key=val HTTP/1.1\r\nhost: {addr}\r\n\r\n",
        reply: REPLY_OK,

    client:
        request:
            method: GET,
            url: "http://{addr}/foo?key=val#dont_send_me",
            headers: {},
            body: None,
        response:
            status: OK,
            headers: {
                "Content-Length" => "0",
            },
            body: None,
}

test! {
    name: client_get_implicitly_empty,

    server:
        expected: "GET / HTTP/1.1\r\nhost: {addr}\r\n\r\n",
        reply: REPLY_OK,

    client:
        request:
            method: GET,
            url: "http://{addr}/",
            headers: {},
            body: Some(""),
        response:
            status: OK,
            headers: {
                "Content-Length" => "0",
            },
            body: None,
}

test! {
    name: client_post_sized,

    server:
        expected: "\
            POST /length HTTP/1.1\r\n\
            content-length: 7\r\n\
            host: {addr}\r\n\
            \r\n\
            foo bar\
            ",
        reply: REPLY_OK,

    client:
        request:
            method: POST,
            url: "http://{addr}/length",
            headers: {
                "Content-Length" => "7",
            },
            body: Some("foo bar"),
        response:
            status: OK,
            headers: {},
            body: None,
}

test! {
    name: client_post_chunked,

    server:
        expected: "\
            POST /chunks HTTP/1.1\r\n\
            transfer-encoding: chunked\r\n\
            host: {addr}\r\n\
            \r\n\
            B\r\n\
            foo bar baz\r\n\
            0\r\n\r\n\
            ",
        reply: REPLY_OK,

    client:
        request:
            method: POST,
            url: "http://{addr}/chunks",
            headers: {
                "Transfer-Encoding" => "chunked",
            },
            body: Some("foo bar baz"),
        response:
            status: OK,
            headers: {},
            body: None,
}

test! {
    name: client_post_empty,

    server:
        expected: "\
            POST /empty HTTP/1.1\r\n\
            content-length: 0\r\n\
            host: {addr}\r\n\
            \r\n\
            ",
        reply: REPLY_OK,

    client:
        request:
            method: POST,
            url: "http://{addr}/empty",
            headers: {
                "Content-Length" => "0",
            },
            body: None,
        response:
            status: OK,
            headers: {},
            body: None,
}

/*TODO: when new Connect trait allows stating connection is proxied
test! {
    name: client_http_proxy,

    server:
        expected: "\
            GET http://{addr}/proxy HTTP/1.1\r\n\
            host: {addr}\r\n\
            \r\n\
            ",
        reply: REPLY_OK,

    client:
        proxy: true,
        request:
            method: GET,
            url: "http://{addr}/proxy",
            headers: {},
            body: None,
        response:
            status: OK,
            headers: {},
            body: None,
}
*/

test! {
    name: client_head_ignores_body,

    server:
        expected: "\
            HEAD /head HTTP/1.1\r\n\
            host: {addr}\r\n\
            \r\n\
            ",
        reply: "\
            HTTP/1.1 200 OK\r\n\
            content-Length: 11\r\n\
            \r\n\
            Hello World\
            ",

    client:
        request:
            method: HEAD,
            url: "http://{addr}/head",
            headers: {},
            body: None,
        response:
            status: OK,
            headers: {},
            body: None,
}

test! {
    name: client_pipeline_responses_extra,

    server:
        expected: "\
            GET /pipe HTTP/1.1\r\n\
            host: {addr}\r\n\
            \r\n\
            ",
        reply: "\
            HTTP/1.1 200 OK\r\n\
            Content-Length: 0\r\n\
            \r\n\
            HTTP/1.1 200 OK\r\n\
            Content-Length: 0\r\n\
            \r\n\
            ",

    client:
        request:
            method: GET,
            url: "http://{addr}/pipe",
            headers: {},
            body: None,
        response:
            status: OK,
            headers: {},
            body: None,
}


test! {
    name: client_error_unexpected_eof,

    server:
        expected: "\
            GET /err HTTP/1.1\r\n\
            host: {addr}\r\n\
            \r\n\
            ",
        reply: "\
            HTTP/1.1 200 OK\r\n\
            ", // unexpected eof before double CRLF

    client:
        request:
            method: GET,
            url: "http://{addr}/err",
            headers: {},
            body: None,
        error: |err| match err {
            &hyper::Error::Incomplete => true,
            _ => false,
        },
}

test! {
    name: client_error_parse_version,

    server:
        expected: "\
            GET /err HTTP/1.1\r\n\
            host: {addr}\r\n\
            \r\n\
            ",
        reply: "\
            HEAT/1.1 200 OK\r\n\
            \r\n\
            ",

    client:
        request:
            method: GET,
            url: "http://{addr}/err",
            headers: {},
            body: None,
        error: |err| match err {
            &hyper::Error::Version => true,
            _ => false,
        },

}

test! {
    name: client_100_continue,

    server:
        expected: "\
            POST /continue HTTP/1.1\r\n\
            content-length: 7\r\n\
            host: {addr}\r\n\
            \r\n\
            foo bar\
            ",
        reply: "\
            HTTP/1.1 100 Continue\r\n\
            \r\n\
            HTTP/1.1 200 OK\r\n\
            Content-Length: 0\r\n\
            \r\n\
            ",

    client:
        request:
            method: POST,
            url: "http://{addr}/continue",
            headers: {
                "Content-Length" => "7",
            },
            body: Some("foo bar"),
        response:
            status: OK,
            headers: {},
            body: None,
}


test! {
    name: client_101_upgrade,

    server:
        expected: "\
            GET /upgrade HTTP/1.1\r\n\
            host: {addr}\r\n\
            \r\n\
            ",
        reply: "\
            HTTP/1.1 101 Switching Protocols\r\n\
            Upgrade: websocket\r\n\
            Connection: upgrade\r\n\
            \r\n\
            ",

    client:
        request:
            method: GET,
            url: "http://{addr}/upgrade",
            headers: {},
            body: None,
        error: |err| match err {
            &hyper::Error::Upgrade => true,
            _ => false,
        },

}

test! {
    name: client_connect_method,

    server:
        expected: "\
            CONNECT {addr} HTTP/1.1\r\n\
            Host: {addr}\r\n\
            \r\n\
            ",
        // won't ever get to reply
        reply: "",

    client:
        request:
            method: CONNECT,
            url: "http://{addr}/",
            headers: {},
            body: None,
        error: |err| match err {
            &hyper::Error::Method => true,
            _ => false,
        },

}

test! {
    name: client_set_host_false,

    server:
        // {addr} is here because format! requires it to exist in the string
        expected: "\
            GET /no-host/{addr} HTTP/1.1\r\n\
            \r\n\
            ",
        reply: "\
            HTTP/1.1 200 OK\r\n\
            Content-Length: 0\r\n\
            \r\n\
            ",

    client:
        set_host: false,
        request:
            method: GET,
            url: "http://{addr}/no-host/{addr}",
            headers: {},
            body: None,
        response:
            status: OK,
            headers: {},
            body: None,
}

mod dispatch_impl {
    use super::*;
    use std::io::{self, Read, Write};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;
    use std::time::Duration;

    use futures::{self, Future};
    use futures::sync::{mpsc, oneshot};
    use tokio_core::reactor::{Timeout};
    use tokio_core::net::TcpStream;
    use tokio_io::{AsyncRead, AsyncWrite};

    use hyper::client::HttpConnector;
    use hyper::server::Service;
    use hyper::{Client, Uri};
    use hyper;



    #[test]
    fn drop_body_before_eof_closes_connection() {
        // https://github.com/hyperium/hyper/issues/1353
        let _ = pretty_env_logger::try_init();

        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = server.local_addr().unwrap();
        let mut core = Core::new().unwrap();
        let handle = core.handle();
        let (closes_tx, closes) = mpsc::channel(10);
        let client = Client::configure()
            .connector(DebugConnector::with_http_and_closes(HttpConnector::new(1, &core.handle()), closes_tx))
            .build(&handle);

        let (tx1, rx1) = oneshot::channel();

        thread::spawn(move || {
            let mut sock = server.accept().unwrap().0;
            sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            sock.set_write_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut buf = [0; 4096];
            sock.read(&mut buf).expect("read 1");
            let body = vec![b'x'; 1024 * 128];
            write!(sock, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n", body.len()).expect("write head");
            let _ = sock.write_all(&body);
            let _ = tx1.send(());
        });

        let req = Request::builder()
            .uri(&*format!("http://{}/a", addr))
            .body(Body::empty())
            .unwrap();
        let res = client.request(req).and_then(move |res| {
            assert_eq!(res.status(), hyper::StatusCode::OK);
            Timeout::new(Duration::from_secs(1), &handle).unwrap()
                .from_err()
        });
        let rx = rx1.map_err(|_| hyper::Error::Io(io::Error::new(io::ErrorKind::Other, "thread panicked")));
        core.run(res.join(rx).map(|r| r.0)).unwrap();

        core.run(closes.into_future()).unwrap().0.expect("closes");
    }

    #[test]
    fn dropped_client_closes_connection() {
        // https://github.com/hyperium/hyper/issues/1353
        let _ = pretty_env_logger::try_init();

        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = server.local_addr().unwrap();
        let mut core = Core::new().unwrap();
        let handle = core.handle();
        let (closes_tx, closes) = mpsc::channel(10);

        let (tx1, rx1) = oneshot::channel();

        thread::spawn(move || {
            let mut sock = server.accept().unwrap().0;
            sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            sock.set_write_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut buf = [0; 4096];
            sock.read(&mut buf).expect("read 1");
            let body =[b'x'; 64];
            write!(sock, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n", body.len()).expect("write head");
            let _ = sock.write_all(&body);
            let _ = tx1.send(());
        });

        let res = {
            let client = Client::configure()
                .connector(DebugConnector::with_http_and_closes(HttpConnector::new(1, &handle), closes_tx))
                .build(&handle);

            let req = Request::builder()
                .uri(&*format!("http://{}/a", addr))
                .body(Body::empty())
                .unwrap();
            client.request(req).and_then(move |res| {
                assert_eq!(res.status(), hyper::StatusCode::OK);
                res.into_body().into_stream().concat2()
            }).and_then(|_| {
                Timeout::new(Duration::from_secs(1), &handle).unwrap()
                    .from_err()
            })
        };
        // client is dropped
        let rx = rx1.map_err(|_| hyper::Error::Io(io::Error::new(io::ErrorKind::Other, "thread panicked")));
        core.run(res.join(rx).map(|r| r.0)).unwrap();

        core.run(closes.into_future()).unwrap().0.expect("closes");
    }


    #[test]
    fn drop_client_closes_idle_connections() {
        let _ = pretty_env_logger::try_init();

        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = server.local_addr().unwrap();
        let mut core = Core::new().unwrap();
        let handle = core.handle();
        let (closes_tx, mut closes) = mpsc::channel(10);

        let (tx1, rx1) = oneshot::channel();
        let (_client_drop_tx, client_drop_rx) = oneshot::channel::<()>();

        thread::spawn(move || {
            let mut sock = server.accept().unwrap().0;
            sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            sock.set_write_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut buf = [0; 4096];
            sock.read(&mut buf).expect("read 1");
            let body =[b'x'; 64];
            write!(sock, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n", body.len()).expect("write head");
            let _ = sock.write_all(&body);
            let _ = tx1.send(());

            // prevent this thread from closing until end of test, so the connection
            // stays open and idle until Client is dropped
            let _ = client_drop_rx.wait();
        });

        let client = Client::configure()
            .connector(DebugConnector::with_http_and_closes(HttpConnector::new(1, &handle), closes_tx))
            .build(&handle);

        let req = Request::builder()
            .uri(&*format!("http://{}/a", addr))
            .body(Body::empty())
            .unwrap();
        let res = client.request(req).and_then(move |res| {
            assert_eq!(res.status(), hyper::StatusCode::OK);
            res.into_body().into_stream().concat2()
        });
        let rx = rx1.map_err(|_| hyper::Error::Io(io::Error::new(io::ErrorKind::Other, "thread panicked")));
        core.run(res.join(rx).map(|r| r.0)).unwrap();

        // not closed yet, just idle
        {
            futures::future::poll_fn(|| {
                assert!(closes.poll()?.is_not_ready());
                Ok::<_, ()>(().into())
            }).wait().unwrap();
        }
        drop(client);

        let t = Timeout::new(Duration::from_millis(100), &handle).unwrap()
            .map(|_| panic!("time out"));
        let close = closes.into_future()
            .map(|(opt, _)| {
                opt.expect("closes");
            })
            .map_err(|_| panic!("closes dropped"));
        let _ = core.run(t.select(close));
    }


    #[test]
    fn drop_response_future_closes_in_progress_connection() {
        let _ = pretty_env_logger::try_init();

        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = server.local_addr().unwrap();
        let mut core = Core::new().unwrap();
        let handle = core.handle();
        let (closes_tx, closes) = mpsc::channel(10);

        let (tx1, rx1) = oneshot::channel();
        let (_client_drop_tx, client_drop_rx) = oneshot::channel::<()>();

        thread::spawn(move || {
            let mut sock = server.accept().unwrap().0;
            sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            sock.set_write_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut buf = [0; 4096];
            sock.read(&mut buf).expect("read 1");
            // we never write a response head
            // simulates a slow server operation
            let _ = tx1.send(());

            // prevent this thread from closing until end of test, so the connection
            // stays open and idle until Client is dropped
            let _ = client_drop_rx.wait();
        });

        let res = {
            let client = Client::configure()
                .connector(DebugConnector::with_http_and_closes(HttpConnector::new(1, &handle), closes_tx))
                .build(&handle);

            let req = Request::builder()
                .uri(&*format!("http://{}/a", addr))
                .body(Body::empty())
                .unwrap();
            client.request(req)
        };

        //let rx = rx1.map_err(|_| hyper::Error::Io(io::Error::new(io::ErrorKind::Other, "thread panicked")));
        core.run(res.select2(rx1)).unwrap();
        // res now dropped
        let t = Timeout::new(Duration::from_millis(100), &handle).unwrap()
            .map(|_| panic!("time out"));
        let close = closes.into_future()
            .map(|(opt, _)| {
                opt.expect("closes");
            })
            .map_err(|_| panic!("closes dropped"));
        let _ = core.run(t.select(close));
    }

    #[test]
    fn drop_response_body_closes_in_progress_connection() {
        let _ = pretty_env_logger::try_init();

        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = server.local_addr().unwrap();
        let mut core = Core::new().unwrap();
        let handle = core.handle();
        let (closes_tx, closes) = mpsc::channel(10);

        let (tx1, rx1) = oneshot::channel();
        let (_client_drop_tx, client_drop_rx) = oneshot::channel::<()>();

        thread::spawn(move || {
            let mut sock = server.accept().unwrap().0;
            sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            sock.set_write_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut buf = [0; 4096];
            sock.read(&mut buf).expect("read 1");
            write!(sock, "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n").expect("write head");
            let _ = tx1.send(());

            // prevent this thread from closing until end of test, so the connection
            // stays open and idle until Client is dropped
            let _ = client_drop_rx.wait();
        });

        let res = {
            let client = Client::configure()
                .connector(DebugConnector::with_http_and_closes(HttpConnector::new(1, &handle), closes_tx))
                .build(&handle);

            let req = Request::builder()
                .uri(&*format!("http://{}/a", addr))
                .body(Body::empty())
                .unwrap();
            // notably, havent read body yet
            client.request(req)
        };

        let rx = rx1.map_err(|_| hyper::Error::Io(io::Error::new(io::ErrorKind::Other, "thread panicked")));
        core.run(res.join(rx).map(|r| r.0)).unwrap();

        let t = Timeout::new(Duration::from_millis(100), &handle).unwrap()
            .map(|_| panic!("time out"));
        let close = closes.into_future()
            .map(|(opt, _)| {
                opt.expect("closes");
            })
            .map_err(|_| panic!("closes dropped"));
        let _ = core.run(t.select(close));
    }

    #[test]
    fn no_keep_alive_closes_connection() {
        // https://github.com/hyperium/hyper/issues/1383
        let _ = pretty_env_logger::try_init();

        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = server.local_addr().unwrap();
        let mut core = Core::new().unwrap();
        let handle = core.handle();
        let (closes_tx, closes) = mpsc::channel(10);

        let (tx1, rx1) = oneshot::channel();
        let (_tx2, rx2) = oneshot::channel::<()>();

        thread::spawn(move || {
            let mut sock = server.accept().unwrap().0;
            sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            sock.set_write_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut buf = [0; 4096];
            sock.read(&mut buf).expect("read 1");
            sock.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n").unwrap();
            let _ = tx1.send(());
            let _ = rx2.wait();
        });

        let client = Client::configure()
            .connector(DebugConnector::with_http_and_closes(HttpConnector::new(1, &handle), closes_tx))
            .keep_alive(false)
            .build(&handle);

        let req = Request::builder()
            .uri(&*format!("http://{}/a", addr))
            .body(Body::empty())
            .unwrap();
        let res = client.request(req).and_then(move |res| {
            assert_eq!(res.status(), hyper::StatusCode::OK);
            res.into_body().into_stream().concat2()
        });
        let rx = rx1.map_err(|_| hyper::Error::Io(io::Error::new(io::ErrorKind::Other, "thread panicked")));
        core.run(res.join(rx).map(|r| r.0)).unwrap();

        let t = Timeout::new(Duration::from_millis(100), &handle).unwrap()
            .map(|_| panic!("time out"));
        let close = closes.into_future()
            .map(|(opt, _)| {
                opt.expect("closes");
            })
            .map_err(|_| panic!("closes dropped"));
        let _ = core.run(t.select(close));
    }

    #[test]
    fn socket_disconnect_closes_idle_conn() {
        // notably when keep-alive is enabled
        let _ = pretty_env_logger::try_init();

        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = server.local_addr().unwrap();
        let mut core = Core::new().unwrap();
        let handle = core.handle();
        let (closes_tx, closes) = mpsc::channel(10);

        let (tx1, rx1) = oneshot::channel();

        thread::spawn(move || {
            let mut sock = server.accept().unwrap().0;
            sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            sock.set_write_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut buf = [0; 4096];
            sock.read(&mut buf).expect("read 1");
            sock.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n").unwrap();
            let _ = tx1.send(());
        });

        let client = Client::configure()
            .connector(DebugConnector::with_http_and_closes(HttpConnector::new(1, &handle), closes_tx))
            .build(&handle);

        let req = Request::builder()
            .uri(&*format!("http://{}/a", addr))
            .body(Body::empty())
            .unwrap();
        let res = client.request(req).and_then(move |res| {
            assert_eq!(res.status(), hyper::StatusCode::OK);
            res.into_body().into_stream().concat2()
        });
        let rx = rx1.map_err(|_| hyper::Error::Io(io::Error::new(io::ErrorKind::Other, "thread panicked")));
        core.run(res.join(rx).map(|r| r.0)).unwrap();


        let t = Timeout::new(Duration::from_millis(100), &handle).unwrap()
            .map(|_| panic!("time out"));
        let close = closes.into_future()
            .map(|(opt, _)| {
                opt.expect("closes");
            })
            .map_err(|_| panic!("closes dropped"));
        let _ = core.run(t.select(close));
    }

    #[test]
    fn conn_drop_prevents_pool_checkout() {
        // a drop might happen for any sort of reason, and we can protect
        // against a lot of them, but if the `Core` is dropped, we can't
        // really catch that. So, this is case to always check.
        //
        // See https://github.com/hyperium/hyper/issues/1429

        use std::error::Error;
        let _ = pretty_env_logger::try_init();

        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = server.local_addr().unwrap();
        let mut core = Core::new().unwrap();
        let handle = core.handle();

        let (tx1, rx1) = oneshot::channel();

        thread::spawn(move || {
            let mut sock = server.accept().unwrap().0;
            sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            sock.set_write_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut buf = [0; 4096];
            sock.read(&mut buf).expect("read 1");
            sock.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n").unwrap();
            sock.read(&mut buf).expect("read 2");
            let _ = tx1.send(());
        });

        let uri = format!("http://{}/a", addr).parse::<hyper::Uri>().unwrap();

        let client = Client::new(&handle);

        let req = Request::builder()
            .uri(uri.clone())
            .body(Body::empty())
            .unwrap();
        let res = client.request(req).and_then(move |res| {
            assert_eq!(res.status(), hyper::StatusCode::OK);
            res.into_body().into_stream().concat2()
        });

        core.run(res).unwrap();

        // drop previous Core
        core = Core::new().unwrap();
        let timeout = Timeout::new(Duration::from_millis(200), &core.handle()).unwrap();
        let rx = rx1.map_err(|_| hyper::Error::Io(io::Error::new(io::ErrorKind::Other, "thread panicked")));
        let rx = rx.and_then(move |_| timeout.map_err(|e| e.into()));

        let req = Request::builder()
            .uri(uri)
            .body(Body::empty())
            .unwrap();
        let res = client.request(req);
        // this does trigger an 'event loop gone' error, but before, it would
        // panic internally on a `SendError`, which is what we're testing against.
        let err = core.run(res.join(rx).map(|r| r.0)).unwrap_err();
        assert_eq!(err.description(), "event loop gone");
    }

    #[test]
    fn client_custom_executor() {
        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = server.local_addr().unwrap();
        let mut core = Core::new().unwrap();
        let handle = core.handle();
        let (closes_tx, closes) = mpsc::channel(10);

        let (tx1, rx1) = oneshot::channel();

        thread::spawn(move || {
            let mut sock = server.accept().unwrap().0;
            sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            sock.set_write_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut buf = [0; 4096];
            sock.read(&mut buf).expect("read 1");
            sock.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n").unwrap();
            let _ = tx1.send(());
        });

        let client = Client::configure()
            .connector(DebugConnector::with_http_and_closes(HttpConnector::new(1, &handle), closes_tx))
            .executor(handle.clone());

        let req = Request::builder()
            .uri(&*format!("http://{}/a", addr))
            .body(Body::empty())
            .unwrap();
        let res = client.request(req).and_then(move |res| {
            assert_eq!(res.status(), hyper::StatusCode::OK);
            res.into_body().into_stream().concat2()
        });
        let rx = rx1.map_err(|_| hyper::Error::Io(io::Error::new(io::ErrorKind::Other, "thread panicked")));

        let timeout = Timeout::new(Duration::from_millis(200), &handle).unwrap();
        let rx = rx.and_then(move |_| timeout.map_err(|e| e.into()));
        core.run(res.join(rx).map(|r| r.0)).unwrap();


        let t = Timeout::new(Duration::from_millis(100), &handle).unwrap()
            .map(|_| panic!("time out"));
        let close = closes.into_future()
            .map(|(opt, _)| {
                opt.expect("closes");
            })
            .map_err(|_| panic!("closes dropped"));
        let _ = core.run(t.select(close));
    }

    #[test]
    fn connect_call_is_lazy() {
        // We especially don't want connects() triggered if there's
        // idle connections that the Checkout would have found
        let _ = pretty_env_logger::try_init();

        let core = Core::new().unwrap();
        let handle = core.handle();
        let connector = DebugConnector::new(&handle);
        let connects = connector.connects.clone();

        let client = Client::configure()
            .connector(connector)
            .build(&handle);

        assert_eq!(connects.load(Ordering::Relaxed), 0);
        let req = Request::builder()
            .uri("http://hyper.local/a")
            .body(Default::default())
            .unwrap();
        let _fut = client.request(req);
        // internal Connect::connect should have been lazy, and not
        // triggered an actual connect yet.
        assert_eq!(connects.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn client_keep_alive_0() {
        let _ = pretty_env_logger::try_init();
        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = server.local_addr().unwrap();
        let mut core = Core::new().unwrap();
        let handle = core.handle();
        let connector = DebugConnector::new(&handle);
        let connects = connector.connects.clone();

        let client = Client::configure()
            .connector(connector)
            .build(&handle);

        let (tx1, rx1) = oneshot::channel();
        let (tx2, rx2) = oneshot::channel();
        thread::spawn(move || {
            let mut sock = server.accept().unwrap().0;
            //drop(server);
            sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            sock.set_write_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut buf = [0; 4096];
            sock.read(&mut buf).expect("read 1");
            sock.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n").expect("write 1");
            let _ = tx1.send(());

            let n2 = sock.read(&mut buf).expect("read 2");
            assert_ne!(n2, 0);
            let second_get = "GET /b HTTP/1.1\r\n";
            assert_eq!(s(&buf[..second_get.len()]), second_get);
            sock.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n").expect("write 2");
            let _ = tx2.send(());
        });


        assert_eq!(connects.load(Ordering::Relaxed), 0);

        let rx = rx1.map_err(|_| hyper::Error::Io(io::Error::new(io::ErrorKind::Other, "thread panicked")));
        let req = Request::builder()
            .uri(&*format!("http://{}/a", addr))
            .body(Body::empty())
            .unwrap();
        let res = client.request(req);
        core.run(res.join(rx).map(|r| r.0)).unwrap();

        assert_eq!(connects.load(Ordering::Relaxed), 1);

        let rx = rx2.map_err(|_| hyper::Error::Io(io::Error::new(io::ErrorKind::Other, "thread panicked")));
        let req = Request::builder()
            .uri(&*format!("http://{}/b", addr))
            .body(Body::empty())
            .unwrap();
        let res = client.request(req);
        core.run(res.join(rx).map(|r| r.0)).unwrap();

        assert_eq!(connects.load(Ordering::Relaxed), 1, "second request should still only have 1 connect");
    }

    #[test]
    fn client_keep_alive_extra_body() {
        let _ = pretty_env_logger::try_init();
        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = server.local_addr().unwrap();
        let mut core = Core::new().unwrap();
        let handle = core.handle();

        let connector = DebugConnector::new(&handle);
        let connects = connector.connects.clone();

        let client = Client::configure()
            .connector(connector)
            .build(&handle);

        let (tx1, rx1) = oneshot::channel();
        let (tx2, rx2) = oneshot::channel();
        thread::spawn(move || {
            let mut sock = server.accept().unwrap().0;
            sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            sock.set_write_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut buf = [0; 4096];
            sock.read(&mut buf).expect("read 1");
            sock.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nhello").expect("write 1");
            // the body "hello", while ignored because its a HEAD request, should mean the connection
            // cannot be put back in the pool
            let _ = tx1.send(());

            let mut sock2 = server.accept().unwrap().0;
            let n2 = sock2.read(&mut buf).expect("read 2");
            assert_ne!(n2, 0);
            let second_get = "GET /b HTTP/1.1\r\n";
            assert_eq!(s(&buf[..second_get.len()]), second_get);
            sock2.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n").expect("write 2");
            let _ = tx2.send(());
        });


        assert_eq!(connects.load(Ordering::Relaxed), 0);

        let rx = rx1.map_err(|_| hyper::Error::Io(io::Error::new(io::ErrorKind::Other, "thread panicked")));
        let req = Request::builder()
            .method("HEAD")
            .uri(&*format!("http://{}/a", addr))
            .body(Body::empty())
            .unwrap();
        let res = client.request(req);
        core.run(res.join(rx).map(|r| r.0)).unwrap();

        assert_eq!(connects.load(Ordering::Relaxed), 1);

        let rx = rx2.map_err(|_| hyper::Error::Io(io::Error::new(io::ErrorKind::Other, "thread panicked")));
        let req = Request::builder()
            .uri(&*format!("http://{}/b", addr))
            .body(Body::empty())
            .unwrap();
        let res = client.request(req);
        core.run(res.join(rx).map(|r| r.0)).unwrap();

        assert_eq!(connects.load(Ordering::Relaxed), 2);
    }


    struct DebugConnector {
        http: HttpConnector,
        closes: mpsc::Sender<()>,
        connects: Arc<AtomicUsize>,
    }

    impl DebugConnector {
        fn new(handle: &Handle) -> DebugConnector {
            let http = HttpConnector::new(1, handle);
            let (tx, _) = mpsc::channel(10);
            DebugConnector::with_http_and_closes(http, tx)
        }

        fn with_http_and_closes(http: HttpConnector, closes: mpsc::Sender<()>) -> DebugConnector {
            DebugConnector {
                http: http,
                closes: closes,
                connects: Arc::new(AtomicUsize::new(0)),
            }
        }
    }

    impl Service for DebugConnector {
        type Request = Uri;
        type Response = DebugStream;
        type Error = io::Error;
        type Future = Box<Future<Item = DebugStream, Error = io::Error>>;

        fn call(&self, uri: Uri) -> Self::Future {
            self.connects.fetch_add(1, Ordering::SeqCst);
            let closes = self.closes.clone();
            Box::new(self.http.call(uri).map(move |s| {
                DebugStream(s, closes)
            }))
        }
    }

    struct DebugStream(TcpStream, mpsc::Sender<()>);

    impl Drop for DebugStream {
        fn drop(&mut self) {
            let _ = self.1.try_send(());
        }
    }

    impl Write for DebugStream {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.write(buf)
        }

        fn flush(&mut self) -> io::Result<()> {
            self.0.flush()
        }
    }

    impl AsyncWrite for DebugStream {
        fn shutdown(&mut self) -> futures::Poll<(), io::Error> {
            AsyncWrite::shutdown(&mut self.0)
        }

        fn write_buf<B: ::bytes::Buf>(&mut self, buf: &mut B) -> futures::Poll<usize, io::Error> {
            self.0.write_buf(buf)
        }
    }

    impl Read for DebugStream {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            self.0.read(buf)
        }
    }

    impl AsyncRead for DebugStream {}
}

mod conn {
    use std::io::{self, Read, Write};
    use std::net::TcpListener;
    use std::thread;
    use std::time::Duration;

    use futures::{Async, Future, Poll, Stream};
    use futures::future::poll_fn;
    use futures::sync::oneshot;
    use tokio_core::reactor::{Core, Timeout};
    use tokio_core::net::TcpStream;
    use tokio_io::{AsyncRead, AsyncWrite};

    use hyper::{self, Request};
    use hyper::client::conn;

    use super::s;

    #[test]
    fn get() {
        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = server.local_addr().unwrap();
        let mut core = Core::new().unwrap();
        let handle = core.handle();

        let (tx1, rx1) = oneshot::channel();

        thread::spawn(move || {
            let mut sock = server.accept().unwrap().0;
            sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            sock.set_write_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut buf = [0; 4096];
            let n = sock.read(&mut buf).expect("read 1");

            // Notably:
            // - Just a path, since just a path was set
            // - No host, since no host was set
            let expected = "GET /a HTTP/1.1\r\n\r\n";
            assert_eq!(s(&buf[..n]), expected);

            sock.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n").unwrap();
            let _ = tx1.send(());
        });

        let tcp = core.run(TcpStream::connect(&addr, &handle)).unwrap();

        let (mut client, conn) = core.run(conn::handshake(tcp)).unwrap();

        handle.spawn(conn.map(|_| ()).map_err(|e| panic!("conn error: {}", e)));

        let req = Request::builder()
            .uri("/a")
            .body(Default::default())
            .unwrap();
        let res = client.send_request(req).and_then(move |res| {
            assert_eq!(res.status(), hyper::StatusCode::OK);
            res.into_body().into_stream().concat2()
        });
        let rx = rx1.map_err(|_| hyper::Error::Io(io::Error::new(io::ErrorKind::Other, "thread panicked")));

        let timeout = Timeout::new(Duration::from_millis(200), &handle).unwrap();
        let rx = rx.and_then(move |_| timeout.map_err(|e| e.into()));
        core.run(res.join(rx).map(|r| r.0)).unwrap();
    }

    #[test]
    fn uri_absolute_form() {
        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = server.local_addr().unwrap();
        let mut core = Core::new().unwrap();
        let handle = core.handle();

        let (tx1, rx1) = oneshot::channel();

        thread::spawn(move || {
            let mut sock = server.accept().unwrap().0;
            sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            sock.set_write_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut buf = [0; 4096];
            let n = sock.read(&mut buf).expect("read 1");

            // Notably:
            // - Still no Host header, since it wasn't set
            let expected = "GET http://hyper.local/a HTTP/1.1\r\n\r\n";
            assert_eq!(s(&buf[..n]), expected);

            sock.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n").unwrap();
            let _ = tx1.send(());
        });

        let tcp = core.run(TcpStream::connect(&addr, &handle)).unwrap();

        let (mut client, conn) = core.run(conn::handshake(tcp)).unwrap();

        handle.spawn(conn.map(|_| ()).map_err(|e| panic!("conn error: {}", e)));

        let req = Request::builder()
            .uri("http://hyper.local/a")
            .body(Default::default())
            .unwrap();

        let res = client.send_request(req).and_then(move |res| {
            assert_eq!(res.status(), hyper::StatusCode::OK);
            res.into_body().into_stream().concat2()
        });
        let rx = rx1.map_err(|_| hyper::Error::Io(io::Error::new(io::ErrorKind::Other, "thread panicked")));

        let timeout = Timeout::new(Duration::from_millis(200), &handle).unwrap();
        let rx = rx.and_then(move |_| timeout.map_err(|e| e.into()));
        core.run(res.join(rx).map(|r| r.0)).unwrap();
    }

    #[test]
    fn pipeline() {
        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = server.local_addr().unwrap();
        let mut core = Core::new().unwrap();
        let handle = core.handle();

        let (tx1, rx1) = oneshot::channel();

        thread::spawn(move || {
            let mut sock = server.accept().unwrap().0;
            sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            sock.set_write_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut buf = [0; 4096];
            sock.read(&mut buf).expect("read 1");
            sock.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n").unwrap();

            let _ = tx1.send(());
        });

        let tcp = core.run(TcpStream::connect(&addr, &handle)).unwrap();

        let (mut client, conn) = core.run(conn::handshake(tcp)).unwrap();

        handle.spawn(conn.map(|_| ()).map_err(|e| panic!("conn error: {}", e)));

        let req = Request::builder()
            .uri("/a")
            .body(Default::default())
            .unwrap();
        let res1 = client.send_request(req).and_then(move |res| {
            assert_eq!(res.status(), hyper::StatusCode::OK);
            res.into_body().into_stream().concat2()
        });

        // pipelined request will hit NotReady, and thus should return an Error::Cancel
        let req = Request::builder()
            .uri("/b")
            .body(Default::default())
            .unwrap();
        let res2 = client.send_request(req)
            .then(|result| {
                let err = result.expect_err("res2");
                match err {
                    hyper::Error::Cancel(..) => (),
                    other => panic!("expected Cancel, found {:?}", other),
                }
                Ok(())
            });

        let rx = rx1.map_err(|_| hyper::Error::Io(io::Error::new(io::ErrorKind::Other, "thread panicked")));

        let timeout = Timeout::new(Duration::from_millis(200), &handle).unwrap();
        let rx = rx.and_then(move |_| timeout.map_err(|e| e.into()));
        core.run(res1.join(res2).join(rx).map(|r| r.0)).unwrap();
    }

    #[test]
    fn upgrade() {
        use tokio_io::io::{read_to_end, write_all};
        let _ = ::pretty_env_logger::try_init();

        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = server.local_addr().unwrap();
        let mut core = Core::new().unwrap();
        let handle = core.handle();

        let (tx1, rx1) = oneshot::channel();

        thread::spawn(move || {
            let mut sock = server.accept().unwrap().0;
            sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            sock.set_write_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut buf = [0; 4096];
            sock.read(&mut buf).expect("read 1");
            sock.write_all(b"\
                HTTP/1.1 101 Switching Protocols\r\n\
                Upgrade: foobar\r\n\
                \r\n\
                foobar=ready\
            ").unwrap();
            let _ = tx1.send(());

            let n = sock.read(&mut buf).expect("read 2");
            assert_eq!(&buf[..n], b"foo=bar");
            sock.write_all(b"bar=foo").expect("write 2");
        });

        let tcp = core.run(TcpStream::connect(&addr, &handle)).unwrap();

        let io = DebugStream {
            tcp: tcp,
            shutdown_called: false,
        };

        let (mut client, mut conn) = core.run(conn::handshake(io)).unwrap();

        {
            let until_upgrade = poll_fn(|| {
                conn.poll_without_shutdown()
            });

            let req = Request::builder()
                .uri("/a")
                .body(Default::default())
                .unwrap();
            let res = client.send_request(req).and_then(move |res| {
                assert_eq!(res.status(), hyper::StatusCode::SWITCHING_PROTOCOLS);
                assert_eq!(res.headers()["Upgrade"], "foobar");
                res.into_body().into_stream().concat2()
            });

            let rx = rx1.map_err(|_| hyper::Error::Io(io::Error::new(io::ErrorKind::Other, "thread panicked")));

            let timeout = Timeout::new(Duration::from_millis(200), &handle).unwrap();
            let rx = rx.and_then(move |_| timeout.map_err(|e| e.into()));
            core.run(until_upgrade.join(res).join(rx).map(|r| r.0)).unwrap();

            // should not be ready now
            core.run(poll_fn(|| {
                assert!(client.poll_ready().unwrap().is_not_ready());
                Ok::<_, ()>(Async::Ready(()))
            })).unwrap();
        }

        let parts = conn.into_parts();
        let io = parts.io;
        let buf = parts.read_buf;

        assert_eq!(buf, b"foobar=ready"[..]);
        assert!(!io.shutdown_called, "upgrade shouldn't shutdown AsyncWrite");
        assert!(client.poll_ready().is_err());

        let io = core.run(write_all(io, b"foo=bar")).unwrap().0;
        let vec = core.run(read_to_end(io, vec![])).unwrap().1;
        assert_eq!(vec, b"bar=foo");
    }

    #[test]
    fn connect_method() {
        use tokio_io::io::{read_to_end, write_all};
        let _ = ::pretty_env_logger::try_init();

        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = server.local_addr().unwrap();
        let mut core = Core::new().unwrap();
        let handle = core.handle();

        let (tx1, rx1) = oneshot::channel();

        thread::spawn(move || {
            let mut sock = server.accept().unwrap().0;
            sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            sock.set_write_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut buf = [0; 4096];
            sock.read(&mut buf).expect("read 1");
            sock.write_all(b"\
                HTTP/1.1 200 OK\r\n\
                \r\n\
                foobar=ready\
            ").unwrap();
            let _ = tx1.send(());

            let n = sock.read(&mut buf).expect("read 2");
            assert_eq!(&buf[..n], b"foo=bar", "sock read 2 bytes");
            sock.write_all(b"bar=foo").expect("write 2");
        });

        let tcp = core.run(TcpStream::connect(&addr, &handle)).unwrap();

        let io = DebugStream {
            tcp: tcp,
            shutdown_called: false,
        };

        let (mut client, mut conn) = core.run(conn::handshake(io)).unwrap();

        {
            let until_tunneled = poll_fn(|| {
                conn.poll_without_shutdown()
            });

            let req = Request::builder()
                .method("CONNECT")
                .uri(addr.to_string())
                .body(Default::default())
                .unwrap();
            let res = client.send_request(req)
                .and_then(move |res| {
                    assert_eq!(res.status(), hyper::StatusCode::OK);
                    res.into_body().into_stream().concat2()
                })
                .map(|body| {
                    assert_eq!(body.as_ref(), b"");
                });

            let rx = rx1.map_err(|_| hyper::Error::Io(io::Error::new(io::ErrorKind::Other, "thread panicked")));

            let timeout = Timeout::new(Duration::from_millis(200), &handle).unwrap();
            let rx = rx.and_then(move |_| timeout.map_err(|e| e.into()));
            core.run(until_tunneled.join(res).join(rx).map(|r| r.0)).unwrap();

            // should not be ready now
            core.run(poll_fn(|| {
                assert!(client.poll_ready().unwrap().is_not_ready());
                Ok::<_, ()>(Async::Ready(()))
            })).unwrap();
        }

        let parts = conn.into_parts();
        let io = parts.io;
        let buf = parts.read_buf;

        assert_eq!(buf, b"foobar=ready"[..]);
        assert!(!io.shutdown_called, "tunnel shouldn't shutdown AsyncWrite");
        assert!(client.poll_ready().is_err());

        let io = core.run(write_all(io, b"foo=bar")).unwrap().0;
        let vec = core.run(read_to_end(io, vec![])).unwrap().1;
        assert_eq!(vec, b"bar=foo");
    }

    struct DebugStream {
        tcp: TcpStream,
        shutdown_called: bool,
    }

    impl Write for DebugStream {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.tcp.write(buf)
        }

        fn flush(&mut self) -> io::Result<()> {
            self.tcp.flush()
        }
    }

    impl AsyncWrite for DebugStream {
        fn shutdown(&mut self) -> Poll<(), io::Error> {
            self.shutdown_called = true;
            AsyncWrite::shutdown(&mut self.tcp)
        }
    }

    impl Read for DebugStream {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            self.tcp.read(buf)
        }
    }

    impl AsyncRead for DebugStream {}
}
