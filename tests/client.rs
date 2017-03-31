#![deny(warnings)]
extern crate hyper;
extern crate futures;
extern crate tokio_core;
extern crate pretty_env_logger;

use std::io::{self, Read, Write};
use std::net::TcpListener;
use std::thread;
use std::time::Duration;

use hyper::client::{Client, Request, HttpConnector};
use hyper::{Method, StatusCode};

use futures::Future;
use futures::sync::oneshot;

use tokio_core::reactor::{Core, Handle};

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
            let _ = pretty_env_logger::init();
            let server = TcpListener::bind("127.0.0.1:0").unwrap();
            let addr = server.local_addr().unwrap();
            let mut core = Core::new().unwrap();
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

            thread::spawn(move || {
                let mut inc = server.accept().unwrap().0;
                inc.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
                inc.set_write_timeout(Some(Duration::from_secs(5))).unwrap();
                let expected = format!($server_expected, addr=addr);
                let mut buf = [0; 4096];
                let mut n = 0;
                while n < buf.len() && n < expected.len() {
                    n += inc.read(&mut buf[n..]).unwrap();
                }
                assert_eq!(s(&buf[..n]), expected, "expected is invalid");

                inc.write_all($server_reply.as_ref()).unwrap();
                let _ = tx.send(());
            });

            let rx = rx.map_err(|_| hyper::Error::Io(io::Error::new(io::ErrorKind::Other, "thread panicked")));

            let work = res.join(rx).map(|r| r.0);

            let res = core.run(work).unwrap();
            assert_eq!(res.status(), StatusCode::$client_status, "status is invalid");
            $(
                assert_eq!(res.headers().get(), Some(&$response_headers), "headers are invalid");
            )*
        }
    );
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

        sock.read(&mut buf).expect("read 2");
        let second_get = b"GET /b HTTP/1.1\r\n";
        assert_eq!(&buf[..second_get.len()], second_get);
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


/* TODO: re-enable once retry works, its currently a flaky test
#[test]
fn client_pooled_socket_disconnected() {
    let _ = pretty_env_logger::init();
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
        let remote_addr = sock.peer_addr().unwrap().to_string();
        let out = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{}", remote_addr.len(), remote_addr);
        sock.write_all(out.as_bytes()).expect("write 1");
        drop(sock);
        tx1.send(());

        let mut sock = server.accept().unwrap().0;
        sock.read(&mut buf).expect("read 2");
        let second_get = b"GET /b HTTP/1.1\r\n";
        assert_eq!(&buf[..second_get.len()], second_get);
        let remote_addr = sock.peer_addr().unwrap().to_string();
        let out = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{}", remote_addr.len(), remote_addr);
        sock.write_all(out.as_bytes()).expect("write 2");
        tx2.send(());
    });

    // spin shortly so we receive the hangup on the client socket
    let sleep = Timeout::new(Duration::from_millis(500), &core.handle()).unwrap();
    core.run(sleep).unwrap();

    let rx = rx1.map_err(|_| hyper::Error::Io(io::Error::new(io::ErrorKind::Other, "thread panicked")));
    let res = client.get(format!("http://{}/a", addr).parse().unwrap())
        .and_then(|res| {
            res.body()
                .map(|chunk| chunk.to_vec())
                .collect()
                .map(|vec| vec.concat())
                .map(|vec| String::from_utf8(vec).unwrap())
        });
    let addr1 = core.run(res.join(rx).map(|r| r.0)).unwrap();

    let rx = rx2.map_err(|_| hyper::Error::Io(io::Error::new(io::ErrorKind::Other, "thread panicked")));
    let res = client.get(format!("http://{}/b", addr).parse().unwrap())
        .and_then(|res| {
            res.body()
                .map(|chunk| chunk.to_vec())
                .collect()
                .map(|vec| vec.concat())
                .map(|vec| String::from_utf8(vec).unwrap())
        });
    let addr2 = core.run(res.join(rx).map(|r| r.0)).unwrap();

    assert_ne!(addr1, addr2);
}
*/
