#![deny(warnings)]
extern crate hyper;
extern crate futures;
extern crate tokio_core;
extern crate tokio_io;
extern crate pretty_env_logger;

use std::io::{self, Read, Write};
use std::net::TcpListener;
use std::thread;
use std::time::Duration;

use hyper::client::{Client, Request, HttpConnector};
use hyper::{Method, StatusCode};

use futures::{Future, Stream};
use futures::future::Either;
use futures::sync::oneshot;

use tokio_core::reactor::{Core, Handle, Timeout};

fn client(handle: &Handle) -> Client<HttpConnector> {
    Client::new(handle)
}

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
                headers: [ $($request_headers:expr,)* ],
                body: $request_body:expr,
                proxy: $request_proxy:expr,

            response:
                status: $client_status:ident,
                headers: [ $($response_headers:expr,)* ],
                body: $response_body:expr,
    ) => (
        #[test]
        fn $name() {
            #![allow(unused)]
            use hyper::header::*;
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
                    request:
                        method: $client_method,
                        url: $client_url,
                        headers: [ $($request_headers,)* ],
                        body: $request_body,
                        proxy: $request_proxy,
            }.unwrap();


            assert_eq!(res.status(), StatusCode::$client_status);
            $(
                assert_eq!(res.headers().get(), Some(&$response_headers));
            )*

            let body = core.run(res.body().concat2()).unwrap();

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
                headers: [ $($request_headers:expr,)* ],
                body: $request_body:expr,
                proxy: $request_proxy:expr,

            error: $err:expr,
    ) => (
        #[test]
        fn $name() {
            #![allow(unused)]
            use hyper::header::*;
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
                    request:
                        method: $client_method,
                        url: $client_url,
                        headers: [ $($request_headers,)* ],
                        body: $request_body,
                        proxy: $request_proxy,
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
            request:
                method: $client_method:ident,
                url: $client_url:expr,
                headers: [ $($request_headers:expr,)* ],
                body: $request_body:expr,
                proxy: $request_proxy:expr,
    ) => ({
        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = server.local_addr().unwrap();
        let mut core = $core;
        let client = client(&core.handle());
        let mut req = Request::new(Method::$client_method, format!($client_url, addr=addr).parse().unwrap());
        $(
            req.headers_mut().set($request_headers);
        )*

        if let Some(body) = $request_body {
            let body: &'static str = body;
            req.set_body(body);
        }
        req.set_proxy($request_proxy);

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
        expected: "GET / HTTP/1.1\r\nHost: {addr}\r\n\r\n",
        reply: REPLY_OK,

    client:
        request:
            method: Get,
            url: "http://{addr}/",
            headers: [],
            body: None,
            proxy: false,
        response:
            status: Ok,
            headers: [
                ContentLength(0),
            ],
            body: None,
}

test! {
    name: client_get_query,

    server:
        expected: "GET /foo?key=val HTTP/1.1\r\nHost: {addr}\r\n\r\n",
        reply: REPLY_OK,

    client:
        request:
            method: Get,
            url: "http://{addr}/foo?key=val#dont_send_me",
            headers: [],
            body: None,
            proxy: false,
        response:
            status: Ok,
            headers: [
                ContentLength(0),
            ],
            body: None,
}

test! {
    name: client_post_sized,

    server:
        expected: "\
            POST /length HTTP/1.1\r\n\
            Host: {addr}\r\n\
            Content-Length: 7\r\n\
            \r\n\
            foo bar\
            ",
        reply: REPLY_OK,

    client:
        request:
            method: Post,
            url: "http://{addr}/length",
            headers: [
                ContentLength(7),
            ],
            body: Some("foo bar"),
            proxy: false,
        response:
            status: Ok,
            headers: [],
            body: None,
}

test! {
    name: client_post_chunked,

    server:
        expected: "\
            POST /chunks HTTP/1.1\r\n\
            Host: {addr}\r\n\
            Transfer-Encoding: chunked\r\n\
            \r\n\
            B\r\n\
            foo bar baz\r\n\
            0\r\n\r\n\
            ",
        reply: REPLY_OK,

    client:
        request:
            method: Post,
            url: "http://{addr}/chunks",
            headers: [
                TransferEncoding::chunked(),
            ],
            body: Some("foo bar baz"),
            proxy: false,
        response:
            status: Ok,
            headers: [],
            body: None,
}

test! {
    name: client_post_empty,

    server:
        expected: "\
            POST /empty HTTP/1.1\r\n\
            Host: {addr}\r\n\
            Content-Length: 0\r\n\
            \r\n\
            ",
        reply: REPLY_OK,

    client:
        request:
            method: Post,
            url: "http://{addr}/empty",
            headers: [
                ContentLength(0),
            ],
            body: Some(""),
            proxy: false,
        response:
            status: Ok,
            headers: [],
            body: None,
}

test! {
    name: client_http_proxy,

    server:
        expected: "\
            GET http://{addr}/proxy HTTP/1.1\r\n\
            Host: {addr}\r\n\
            \r\n\
            ",
        reply: REPLY_OK,

    client:
        request:
            method: Get,
            url: "http://{addr}/proxy",
            headers: [],
            body: None,
            proxy: true,
        response:
            status: Ok,
            headers: [],
            body: None,
}


test! {
    name: client_head_ignores_body,

    server:
        expected: "\
            HEAD /head HTTP/1.1\r\n\
            Host: {addr}\r\n\
            \r\n\
            ",
        reply: "\
            HTTP/1.1 200 OK\r\n\
            Content-Length: 11\r\n\
            \r\n\
            Hello World\
            ",

    client:
        request:
            method: Head,
            url: "http://{addr}/head",
            headers: [],
            body: None,
            proxy: false,
        response:
            status: Ok,
            headers: [],
            body: None,
}

test! {
    name: client_pipeline_responses_extra,

    server:
        expected: "\
            GET /pipe HTTP/1.1\r\n\
            Host: {addr}\r\n\
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
            method: Get,
            url: "http://{addr}/pipe",
            headers: [],
            body: None,
            proxy: false,
        response:
            status: Ok,
            headers: [],
            body: None,
}


test! {
    name: client_error_unexpected_eof,

    server:
        expected: "\
            GET /err HTTP/1.1\r\n\
            Host: {addr}\r\n\
            \r\n\
            ",
        reply: "\
            HTTP/1.1 200 OK\r\n\
            ", // unexpected eof before double CRLF

    client:
        request:
            method: Get,
            url: "http://{addr}/err",
            headers: [],
            body: None,
            proxy: false,
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
            Host: {addr}\r\n\
            \r\n\
            ",
        reply: "\
            HEAT/1.1 200 OK\r\n\
            \r\n\
            ",

    client:
        request:
            method: Get,
            url: "http://{addr}/err",
            headers: [],
            body: None,
            proxy: false,
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
            Host: {addr}\r\n\
            Content-Length: 7\r\n\
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
            method: Post,
            url: "http://{addr}/continue",
            headers: [
                ContentLength(7),
            ],
            body: Some("foo bar"),
            proxy: false,
        response:
            status: Ok,
            headers: [],
            body: None,
}


test! {
    name: client_101_upgrade,

    server:
        expected: "\
            GET /upgrade HTTP/1.1\r\n\
            Host: {addr}\r\n\
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
            method: Get,
            url: "http://{addr}/upgrade",
            headers: [],
            body: None,
            proxy: false,
        error: |err| match err {
            &hyper::Error::Upgrade => true,
            _ => false,
        },

}

#[test]
fn client_keep_alive() {
    let server = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = server.local_addr().unwrap();
    let mut core = Core::new().unwrap();
    let client = client(&core.handle());


    let (tx1, rx1) = oneshot::channel();
    let (tx2, rx2) = oneshot::channel();
    thread::spawn(move || {
        let mut sock = server.accept().unwrap().0;
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



    let rx = rx1.map_err(|_| hyper::Error::Io(io::Error::new(io::ErrorKind::Other, "thread panicked")));
    let res = client.get(format!("http://{}/a", addr).parse().unwrap());
    core.run(res.join(rx).map(|r| r.0)).unwrap();

    let rx = rx2.map_err(|_| hyper::Error::Io(io::Error::new(io::ErrorKind::Other, "thread panicked")));
    let res = client.get(format!("http://{}/b", addr).parse().unwrap());
    core.run(res.join(rx).map(|r| r.0)).unwrap();
}

#[test]
fn client_keep_alive_connreset() {
    use std::sync::mpsc;
    extern crate pretty_env_logger;
    let _ = pretty_env_logger::try_init();

    let server = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = server.local_addr().unwrap();
    let mut core = Core::new().unwrap();
    let handle = core.handle();

    // This one seems to hang forever
    let client = client(&handle);

    let (tx1, rx1) = oneshot::channel();
    let (tx2, rx2) = mpsc::channel();
    thread::spawn(move || {
        let mut sock = server.accept().unwrap().0;
        sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        sock.set_write_timeout(Some(Duration::from_secs(5))).unwrap();
        let mut buf = [0; 4096];
        sock.read(&mut buf).expect("read 1");
        sock.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n").expect("write 1");

        // Wait for client to indicate it is done processing the first request
        // This is what seem to trigger the race condition -- without it client notices
        // connection is closed while processing the first request.
        let _ = rx2.recv();
        let _r = sock.shutdown(std::net::Shutdown::Both);

        // Let client know it can try to reuse the connection
        let _ = tx1.send(());
    });


    let res = client.get(format!("http://{}/a", addr).parse().unwrap());
    core.run(res).unwrap();

    let _ = tx2.send(());

    let rx = rx1.map_err(|_| hyper::Error::Io(io::Error::new(io::ErrorKind::Other, "thread panicked")));
    core.run(rx).unwrap();

    let t = Timeout::new(Duration::from_millis(100), &handle).unwrap();
    let res = client.get(format!("http://{}/b", addr).parse().unwrap());
    let fut = res.select2(t).then(|result| match result {
        Ok(Either::A((resp, _))) => Ok(resp),
        Err(Either::A((err, _))) => Err(err),
        Ok(Either::B(_)) |
        Err(Either::B(_)) => Err(hyper::Error::Timeout),
    });

    // for now, the 2nd request is just canceled, since the connection is found to be dead
    // at the same time the request is scheduled.
    //
    // in the future, it'd be nice to auto retry the request, but can't really be done yet
    // as the `connector` isn't clone so we can't use it "later", when the future resolves.
    let err = core.run(fut).unwrap_err();
    match err {
        hyper::Error::Cancel(..) => (),
        other => panic!("expected Cancel error, got {:?}", other),
    }
}

#[test]
fn client_keep_alive_extra_body() {
    let _ = pretty_env_logger::try_init();
    let server = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = server.local_addr().unwrap();
    let mut core = Core::new().unwrap();
    let client = client(&core.handle());


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



    let rx = rx1.map_err(|_| hyper::Error::Io(io::Error::new(io::ErrorKind::Other, "thread panicked")));
    let req = Request::new(Method::Head, format!("http://{}/a", addr).parse().unwrap());
    let res = client.request(req);
    core.run(res.join(rx).map(|r| r.0)).unwrap();

    let rx = rx2.map_err(|_| hyper::Error::Io(io::Error::new(io::ErrorKind::Other, "thread panicked")));
    let res = client.get(format!("http://{}/b", addr).parse().unwrap());
    core.run(res.join(rx).map(|r| r.0)).unwrap();
}

mod dispatch_impl {
    use super::*;
    use std::io::{self, Read, Write};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;
    use std::time::Duration;

    use futures::{self, Future};
    use futures::sync::oneshot;
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
        let closes = Arc::new(AtomicUsize::new(0));
        let client = Client::configure()
            .connector(DebugConnector(HttpConnector::new(1, &core.handle()), closes.clone()))
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

        let uri = format!("http://{}/a", addr).parse().unwrap();

        let res = client.get(uri).and_then(move |res| {
            assert_eq!(res.status(), hyper::StatusCode::Ok);
            Timeout::new(Duration::from_secs(1), &handle).unwrap()
                .from_err()
        });
        let rx = rx1.map_err(|_| hyper::Error::Io(io::Error::new(io::ErrorKind::Other, "thread panicked")));
        core.run(res.join(rx).map(|r| r.0)).unwrap();

        assert_eq!(closes.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn dropped_client_closes_connection() {
        // https://github.com/hyperium/hyper/issues/1353
        let _ = pretty_env_logger::try_init();

        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = server.local_addr().unwrap();
        let mut core = Core::new().unwrap();
        let handle = core.handle();
        let closes = Arc::new(AtomicUsize::new(0));

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

        let uri = format!("http://{}/a", addr).parse().unwrap();

        let res = {
            let client = Client::configure()
                .connector(DebugConnector(HttpConnector::new(1, &handle), closes.clone()))
                .build(&handle);
            client.get(uri).and_then(move |res| {
                assert_eq!(res.status(), hyper::StatusCode::Ok);
                res.body().concat2()
            }).and_then(|_| {
                Timeout::new(Duration::from_secs(1), &handle).unwrap()
                    .from_err()
            })
        };
        // client is dropped
        let rx = rx1.map_err(|_| hyper::Error::Io(io::Error::new(io::ErrorKind::Other, "thread panicked")));
        core.run(res.join(rx).map(|r| r.0)).unwrap();

        assert_eq!(closes.load(Ordering::Relaxed), 1);
    }


    #[test]
    fn drop_client_closes_idle_connections() {
        let _ = pretty_env_logger::try_init();

        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = server.local_addr().unwrap();
        let mut core = Core::new().unwrap();
        let handle = core.handle();
        let closes = Arc::new(AtomicUsize::new(0));

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

        let uri = format!("http://{}/a", addr).parse().unwrap();

        let client = Client::configure()
            .connector(DebugConnector(HttpConnector::new(1, &handle), closes.clone()))
            .build(&handle);
        let res = client.get(uri).and_then(move |res| {
            assert_eq!(res.status(), hyper::StatusCode::Ok);
            res.body().concat2()
        });
        let rx = rx1.map_err(|_| hyper::Error::Io(io::Error::new(io::ErrorKind::Other, "thread panicked")));
        core.run(res.join(rx).map(|r| r.0)).unwrap();

        // not closed yet, just idle
        assert_eq!(closes.load(Ordering::Relaxed), 0);
        drop(client);
        core.run(Timeout::new(Duration::from_millis(100), &handle).unwrap()).unwrap();

        assert_eq!(closes.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn drop_response_future_closes_in_progress_connection() {
        let _ = pretty_env_logger::try_init();

        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = server.local_addr().unwrap();
        let mut core = Core::new().unwrap();
        let handle = core.handle();
        let closes = Arc::new(AtomicUsize::new(0));

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

        let uri = format!("http://{}/a", addr).parse().unwrap();

        let res = {
            let client = Client::configure()
                .connector(DebugConnector(HttpConnector::new(1, &handle), closes.clone()))
                .build(&handle);
            client.get(uri)
        };

        //let rx = rx1.map_err(|_| hyper::Error::Io(io::Error::new(io::ErrorKind::Other, "thread panicked")));
        core.run(res.select2(rx1)).unwrap();
        // res now dropped
        core.run(Timeout::new(Duration::from_millis(100), &handle).unwrap()).unwrap();

        assert_eq!(closes.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn drop_response_body_closes_in_progress_connection() {
        let _ = pretty_env_logger::try_init();

        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = server.local_addr().unwrap();
        let mut core = Core::new().unwrap();
        let handle = core.handle();
        let closes = Arc::new(AtomicUsize::new(0));

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

        let uri = format!("http://{}/a", addr).parse().unwrap();

        let res = {
            let client = Client::configure()
                .connector(DebugConnector(HttpConnector::new(1, &handle), closes.clone()))
                .build(&handle);
            // notably, havent read body yet
            client.get(uri)
        };

        let rx = rx1.map_err(|_| hyper::Error::Io(io::Error::new(io::ErrorKind::Other, "thread panicked")));
        core.run(res.join(rx).map(|r| r.0)).unwrap();
        core.run(Timeout::new(Duration::from_millis(100), &handle).unwrap()).unwrap();

        assert_eq!(closes.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn no_keep_alive_closes_connection() {
        // https://github.com/hyperium/hyper/issues/1383
        let _ = pretty_env_logger::try_init();

        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = server.local_addr().unwrap();
        let mut core = Core::new().unwrap();
        let handle = core.handle();
        let closes = Arc::new(AtomicUsize::new(0));

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

        let uri = format!("http://{}/a", addr).parse().unwrap();

        let client = Client::configure()
            .connector(DebugConnector(HttpConnector::new(1, &handle), closes.clone()))
            .keep_alive(false)
            .build(&handle);
        let res = client.get(uri).and_then(move |res| {
            assert_eq!(res.status(), hyper::StatusCode::Ok);
            res.body().concat2()
        });
        let rx = rx1.map_err(|_| hyper::Error::Io(io::Error::new(io::ErrorKind::Other, "thread panicked")));
        core.run(res.join(rx).map(|r| r.0)).unwrap();

        assert_eq!(closes.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn socket_disconnect_closes_idle_conn() {
        // notably when keep-alive is enabled
        let _ = pretty_env_logger::try_init();

        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = server.local_addr().unwrap();
        let mut core = Core::new().unwrap();
        let handle = core.handle();
        let closes = Arc::new(AtomicUsize::new(0));

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

        let uri = format!("http://{}/a", addr).parse().unwrap();

        let client = Client::configure()
            .connector(DebugConnector(HttpConnector::new(1, &handle), closes.clone()))
            .build(&handle);
        let res = client.get(uri).and_then(move |res| {
            assert_eq!(res.status(), hyper::StatusCode::Ok);
            res.body().concat2()
        });
        let rx = rx1.map_err(|_| hyper::Error::Io(io::Error::new(io::ErrorKind::Other, "thread panicked")));

        let timeout = Timeout::new(Duration::from_millis(200), &handle).unwrap();
        let rx = rx.and_then(move |_| timeout.map_err(|e| e.into()));
        core.run(res.join(rx).map(|r| r.0)).unwrap();

        assert_eq!(closes.load(Ordering::Relaxed), 1);
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
        let res = client.get(uri.clone()).and_then(move |res| {
            assert_eq!(res.status(), hyper::StatusCode::Ok);
            res.body().concat2()
        });

        core.run(res).unwrap();

        // drop previous Core
        core = Core::new().unwrap();
        let timeout = Timeout::new(Duration::from_millis(200), &core.handle()).unwrap();
        let rx = rx1.map_err(|_| hyper::Error::Io(io::Error::new(io::ErrorKind::Other, "thread panicked")));
        let rx = rx.and_then(move |_| timeout.map_err(|e| e.into()));

        let res = client.get(uri);
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
        let closes = Arc::new(AtomicUsize::new(0));

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

        let uri = format!("http://{}/a", addr).parse().unwrap();

        let client = Client::configure()
            .connector(DebugConnector(HttpConnector::new(1, &handle), closes.clone()))
            .executor(handle.clone());
        let res = client.get(uri).and_then(move |res| {
            assert_eq!(res.status(), hyper::StatusCode::Ok);
            res.body().concat2()
        });
        let rx = rx1.map_err(|_| hyper::Error::Io(io::Error::new(io::ErrorKind::Other, "thread panicked")));

        let timeout = Timeout::new(Duration::from_millis(200), &handle).unwrap();
        let rx = rx.and_then(move |_| timeout.map_err(|e| e.into()));
        core.run(res.join(rx).map(|r| r.0)).unwrap();

        assert_eq!(closes.load(Ordering::Relaxed), 1);
    }

    struct DebugConnector(HttpConnector, Arc<AtomicUsize>);

    impl Service for DebugConnector {
        type Request = Uri;
        type Response = DebugStream;
        type Error = io::Error;
        type Future = Box<Future<Item = DebugStream, Error = io::Error>>;

        fn call(&self, uri: Uri) -> Self::Future {
            let counter = self.1.clone();
            Box::new(self.0.call(uri).map(move |s| DebugStream(s, counter)))
        }
    }

    struct DebugStream(TcpStream, Arc<AtomicUsize>);

    impl Drop for DebugStream {
        fn drop(&mut self) {
            self.1.fetch_add(1, Ordering::SeqCst);
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
    }

    impl Read for DebugStream {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            self.0.read(buf)
        }
    }

    impl AsyncRead for DebugStream {}
}
