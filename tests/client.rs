//#![deny(warnings)]
extern crate hyper;
extern crate futures;
extern crate tokio_core;

use std::io::{self, Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;
use std::time::Duration;

use hyper::client::{Client, Request, Response, DefaultConnector};
use hyper::{Method, StatusCode};
use hyper::header::Headers;

use futures::Future;

use tokio_core::reactor::{Core, Handle};

fn s(bytes: &[u8]) -> &str {
    ::std::str::from_utf8(bytes.as_ref()).unwrap()
}


fn client(handle: &Handle) -> Client<DefaultConnector> {
    Client::new(handle).unwrap()
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

            response:
                status: $client_status:ident,
                headers: [ $($response_headers:expr,)* ],
                body: $response_body:expr,
    ) => (
        #[test]
        fn $name() {
            #[allow(unused)]
            use hyper::header::*;
            let server = TcpListener::bind("127.0.0.1:0").unwrap();
            let addr = server.local_addr().unwrap();
            let mut core = Core::new().unwrap();
            let client = client(&core.handle());
            let mut req = Request::new(Method::$client_method, format!($client_url, addr=addr).parse().unwrap());
            $(
                req.headers_mut().set($request_headers);
            )*
            let res = client.request(req);

            ::std::thread::spawn(move || {
                let mut inc = server.accept().unwrap().0;
                inc.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
                inc.set_write_timeout(Some(Duration::from_secs(5))).unwrap();
                let expected = format!($server_expected, addr=addr);
                let mut buf = [0; 4096];
                let mut n = 0;
                while n < buf.len() && n < expected.len() {
                    n += inc.read(&mut buf[n..]).unwrap();
                }
                //assert_eq!(s(&buf[..n]), expected);

                inc.write_all($server_reply.as_ref()).unwrap();
            });

            let res = core.run(res).unwrap();
            assert_eq!(res.status(), &StatusCode::$client_status);
            $(
                assert_eq!(res.headers().get(), Some(&$response_headers));
            )*
            //drop(inc);
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
            body: Some(b"foo bar"),
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
            body: Some(b"foo bar baz"),
        response:
            status: Ok,
            headers: [],
            body: None,
}

/*
#[test]
fn client_read_timeout() {
    let server = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = server.local_addr().unwrap();
    let client = client();
    let res = client.request(format!("http://{}/", addr), opts().read_timeout(Duration::from_secs(3)));

    let mut inc = server.accept().unwrap().0;
    let mut buf = [0; 4096];
    inc.read(&mut buf).unwrap();

    match res.recv() {
        Ok(Msg::Error(hyper::Error::Timeout)) => (),
        other => panic!("expected timeout, actual: {:?}", other)
    }
}
*/

/*
#[test]
fn client_keep_alive() {
    let server = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = server.local_addr().unwrap();
    let client = client();
    let res = client.request(format!("http://{}/a", addr), opts());

    let mut sock = server.accept().unwrap().0;
    sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    sock.set_write_timeout(Some(Duration::from_secs(5))).unwrap();
    let mut buf = [0; 4096];
    sock.read(&mut buf).expect("read 1");
    sock.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n").expect("write 1");

    while let Ok(_) = res.recv() {}

    let res = client.request(format!("http://{}/b", addr), opts());
    sock.read(&mut buf).expect("read 2");
    let second_get = b"GET /b HTTP/1.1\r\n";
    assert_eq!(&buf[..second_get.len()], second_get);
    sock.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n").expect("write 2");

    while let Ok(_) = res.recv() {}
}
*/
