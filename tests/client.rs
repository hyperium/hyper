#![deny(warnings)]
#![warn(rust_2018_idioms)]

#[macro_use]
extern crate matches;

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener};
use std::pin::Pin;
use std::task::{Context, Poll};
use std::thread;
use std::time::Duration;

use hyper::body::to_bytes as concat;
use hyper::{Body, Client, Method, Request, StatusCode};

use futures_channel::oneshot;
use futures_core::{Future, Stream, TryFuture};
use futures_util::future::{self, FutureExt, TryFutureExt};
use tokio::net::TcpStream;
mod support;

fn s(buf: &[u8]) -> &str {
    std::str::from_utf8(buf).expect("from_utf8")
}

fn tcp_connect(addr: &SocketAddr) -> impl Future<Output = std::io::Result<TcpStream>> {
    TcpStream::connect(*addr)
}

macro_rules! test {
    (
        name: $name:ident,
        server:
            expected: $server_expected:expr,
            reply: $server_reply:expr,
        client:
            request: {$(
                $c_req_prop:ident: $c_req_val: tt,
            )*},

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
                request: {$(
                    $c_req_prop: $c_req_val,
                )*},

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
            request: {$(
                $c_req_prop:ident: $c_req_val:tt,
            )*},

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
                set_host: $set_host,
                title_case_headers: false,
                request: {$(
                    $c_req_prop: $c_req_val,
                )*},

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
            title_case_headers: $title_case_headers:expr,
            request: {$(
                $c_req_prop:ident: $c_req_val:tt,
            )*},

            response:
                status: $client_status:ident,
                headers: { $($response_header_name:expr => $response_header_val:expr,)* },
                body: $response_body:expr,
    ) => (
        #[test]
        fn $name() {
            let _ = pretty_env_logger::try_init();
            let rt = support::runtime();

            let res = test! {
                INNER;
                name: $name,
                runtime: &rt,
                server:
                    expected: $server_expected,
                    reply: $server_reply,
                client:
                    set_host: $set_host,
                    title_case_headers: $title_case_headers,
                    request: {$(
                        $c_req_prop: $c_req_val,
                    )*},
            }.expect("test");


            assert_eq!(res.status(), StatusCode::$client_status);
            $(
                assert_eq!(
                    res
                        .headers()
                        .get($response_header_name)
                        .expect(concat!("response header '", stringify!($response_header_name), "'")),
                    $response_header_val,
                    "response header '{}'",
                    stringify!($response_header_name),
                );
            )*

            let body = rt.block_on(concat(res))
                .expect("body concat wait");

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
            request: {$(
                $c_req_prop:ident: $c_req_val:tt,
            )*},

            error: $err:expr,
    ) => (
        #[test]
        fn $name() {
            let _ = pretty_env_logger::try_init();
            let rt = support::runtime();

            let err: ::hyper::Error = test! {
                INNER;
                name: $name,
                runtime: &rt,
                server:
                    expected: $server_expected,
                    reply: $server_reply,
                client:
                    set_host: true,
                    title_case_headers: false,
                    request: {$(
                        $c_req_prop: $c_req_val,
                    )*},
            }.unwrap_err();

            fn infer_closure<F: FnOnce(&::hyper::Error) -> bool>(f: F) -> F { f }

            let closure = infer_closure($err);
            if !closure(&err) {
                panic!("expected error, unexpected variant: {:?}", err);
            }
        }
    );

    (
        INNER;
        name: $name:ident,
        runtime: $runtime:expr,
        server:
            expected: $server_expected:expr,
            reply: $server_reply:expr,
        client:
            set_host: $set_host:expr,
            title_case_headers: $title_case_headers:expr,
            request: {$(
                $c_req_prop:ident: $c_req_val:tt,
            )*},
    ) => ({
        let server = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = server.local_addr().expect("local_addr");
        let rt = $runtime;

        let connector = ::hyper::client::HttpConnector::new();
        let client = Client::builder()
            .set_host($set_host)
            .http1_title_case_headers($title_case_headers)
            .build(connector);

        #[allow(unused_assignments, unused_mut)]
        let mut body = Body::empty();
        let mut req_builder = Request::builder();
        $(
            test!(@client_request; req_builder, body, addr, $c_req_prop: $c_req_val);
        )*
        let req = req_builder
            .body(body)
            .expect("request builder");

        let res = client.request(req);

        let (tx, rx) = oneshot::channel();

        let thread = thread::Builder::new()
            .name(format!("tcp-server<{}>", stringify!($name)));
        thread.spawn(move || {
            let mut inc = server.accept().expect("accept").0;
            inc.set_read_timeout(Some(Duration::from_secs(5))).expect("set_read_timeout");
            inc.set_write_timeout(Some(Duration::from_secs(5))).expect("set_write_timeout");
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

            inc.write_all($server_reply.as_ref()).expect("write_all");
            let _ = tx.send(Ok::<_, hyper::Error>(()));
        }).expect("thread spawn");

        let rx = rx.expect("thread panicked");

        rt.block_on(future::try_join(res, rx).map_ok(|r| r.0)).map(move |mut resp| {
            // Always check that HttpConnector has set the "extra" info...
            let extra = resp
                .extensions_mut()
                .remove::<::hyper::client::connect::HttpInfo>()
                .expect("HttpConnector should set HttpInfo");

            assert_eq!(extra.remote_addr(), addr, "HttpInfo should have server addr");

            resp
        })
    });

    (
        @client_request;
        $req_builder:ident,
        $body:ident,
        $addr:ident,
        $c_req_prop:ident: $c_req_val:tt
    ) => ({
        __client_req_prop!($req_builder, $body, $addr, $c_req_prop: $c_req_val)
    });
}

macro_rules! __client_req_prop {
    ($req_builder:ident, $body:ident, $addr:ident, headers: $map:tt) => {{
        __client_req_header!($req_builder, $map)
    }};

    ($req_builder:ident, $body:ident, $addr:ident, method: $method:ident) => {{
        $req_builder = $req_builder.method(Method::$method);
    }};

    ($req_builder:ident, $body:ident, $addr:ident, version: $version:ident) => {{
        $req_builder = $req_builder.version(hyper::Version::$version);
    }};

    ($req_builder:ident, $body:ident, $addr:ident, url: $url:expr) => {{
        $req_builder = $req_builder.uri(format!($url, addr = $addr));
    }};

    ($req_builder:ident, $body:ident, $addr:ident, body: $body_e:expr) => {{
        $body = $body_e.into();
    }};
}

macro_rules! __client_req_header {
    ($req_builder:ident, { $($name:expr => $val:expr,)* }) => {
        $(
        $req_builder = $req_builder.header($name, $val);
        )*
    }
}

static REPLY_OK: &str = "HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n";

test! {
    name: client_get,

    server:
        expected: "GET / HTTP/1.1\r\nhost: {addr}\r\n\r\n",
        reply: REPLY_OK,

    client:
        request: {
            method: GET,
            url: "http://{addr}/",
        },
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
        request: {
            method: GET,
            url: "http://{addr}/foo?key=val#dont_send_me",
        },
        response:
            status: OK,
            headers: {
                "Content-Length" => "0",
            },
            body: None,
}

test! {
    name: client_get_req_body_implicitly_empty,

    server:
        expected: "GET / HTTP/1.1\r\nhost: {addr}\r\n\r\n",
        reply: REPLY_OK,

    client:
        request: {
            method: GET,
            url: "http://{addr}/",
            body: "", // not Body::empty
        },
        response:
            status: OK,
            headers: {},
            body: None,
}

test! {
    name: client_get_req_body_chunked,

    server:
        expected: "\
            GET / HTTP/1.1\r\n\
            transfer-encoding: chunked\r\n\
            host: {addr}\r\n\
            \r\n\
            5\r\n\
            hello\r\n\
            0\r\n\r\n\
            ",
        reply: REPLY_OK,

    client:
        request: {
            method: GET,
            url: "http://{addr}/",
            headers: {
                "transfer-encoding" => "chunked",
            },
            body: "hello", // not Body::empty
        },
        response:
            status: OK,
            headers: {},
            body: None,
}

test! {
    name: client_transfer_encoding_repair,

    server:
        expected: "\
            GET / HTTP/1.1\r\n\
            transfer-encoding: foo, chunked\r\n\
            host: {addr}\r\n\
            \r\n\
            5\r\n\
            hello\r\n\
            0\r\n\r\n\
            ",
        reply: REPLY_OK,

    client:
        request: {
            method: GET,
            url: "http://{addr}/",
            headers: {
                "transfer-encoding" => "foo",
            },
            body: "hello", // not Body::empty
        },
        response:
            status: OK,
            headers: {},
            body: None,
}

test! {
    name: client_get_req_body_chunked_http10,

    server:
        expected: "\
            GET / HTTP/1.0\r\n\
            host: {addr}\r\n\
            content-length: 5\r\n\
            \r\n\
            hello\
            ",
        reply: "HTTP/1.0 200 OK\r\ncontent-length: 0\r\n\r\n",

    client:
        request: {
            method: GET,
            url: "http://{addr}/",
            headers: {
                "transfer-encoding" => "chunked",
            },
            version: HTTP_10,
            body: "hello",
        },
        response:
            status: OK,
            headers: {},
            body: None,
}

test! {
    name: client_get_req_body_chunked_with_trailer,

    server:
        expected: "\
            GET / HTTP/1.1\r\n\
            host: {addr}\r\n\
            \r\n\
            ",
        reply: "\
            HTTP/1.1 200 OK\r\n\
            Transfer-Encoding: chunked\r\n\
            \r\n\
            5\r\n\
            hello\r\n\
            0\r\n\
            Trailer: value\r\n\
            \r\n\
            ",

    client:
        request: {
            method: GET,
            url: "http://{addr}/",
        },
        response:
            status: OK,
            headers: {},
            body: &b"hello"[..],
}

test! {
    name: client_get_req_body_chunked_with_multiple_trailers,

    server:
        expected: "\
            GET / HTTP/1.1\r\n\
            host: {addr}\r\n\
            \r\n\
            ",
        reply: "\
            HTTP/1.1 200 OK\r\n\
            Transfer-Encoding: chunked\r\n\
            \r\n\
            5\r\n\
            hello\r\n\
            0\r\n\
            Trailer: value\r\n\
            another-trainer: another-value\r\n\
            \r\n\
            ",

    client:
        request: {
            method: GET,
            url: "http://{addr}/",
        },
        response:
            status: OK,
            headers: {},
            body: &b"hello"[..],
}

test! {
    name: client_get_req_body_sized,

    server:
        expected: "\
            GET / HTTP/1.1\r\n\
            content-length: 5\r\n\
            host: {addr}\r\n\
            \r\n\
            hello\
            ",
        reply: REPLY_OK,

    client:
        request: {
            method: GET,
            url: "http://{addr}/",
            headers: {
                "Content-Length" => "5",
            },
            body: (Body::wrap_stream(Body::from("hello"))),
        },
        response:
            status: OK,
            headers: {},
            body: None,
}

test! {
    name: client_get_req_body_unknown,

    server:
        expected: "\
            GET / HTTP/1.1\r\n\
            host: {addr}\r\n\
            \r\n\
            ",
        reply: REPLY_OK,

    client:
        request: {
            method: GET,
            url: "http://{addr}/",
            // wrap_steam means we don't know the content-length,
            // but we're wrapping a non-empty stream.
            //
            // But since the headers cannot tell us, and the method typically
            // doesn't have a body, the body must be ignored.
            body: (Body::wrap_stream(Body::from("hello"))),
        },
        response:
            status: OK,
            headers: {},
            body: None,
}

test! {
    name: client_get_req_body_unknown_http10,

    server:
        expected: "\
            GET / HTTP/1.0\r\n\
            host: {addr}\r\n\
            \r\n\
            ",
        reply: "HTTP/1.0 200 OK\r\ncontent-length: 0\r\n\r\n",

    client:
        request: {
            method: GET,
            url: "http://{addr}/",
            headers: {
                "transfer-encoding" => "chunked",
            },
            version: HTTP_10,
            // wrap_steam means we don't know the content-length,
            // but we're wrapping a non-empty stream.
            //
            // But since the headers cannot tell us, the body must be ignored.
            body: (Body::wrap_stream(Body::from("hello"))),
        },
        response:
            status: OK,
            headers: {},
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
        request: {
            method: POST,
            url: "http://{addr}/length",
            headers: {
                "Content-Length" => "7",
            },
            body: "foo bar",
        },
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
        request: {
            method: POST,
            url: "http://{addr}/chunks",
            headers: {
                "Transfer-Encoding" => "chunked",
            },
            body: "foo bar baz",
        },
        response:
            status: OK,
            headers: {},
            body: None,
}

test! {
    name: client_post_unknown,

    server:
        expected: "\
            POST /chunks HTTP/1.1\r\n\
            host: {addr}\r\n\
            transfer-encoding: chunked\r\n\
            \r\n\
            B\r\n\
            foo bar baz\r\n\
            0\r\n\r\n\
            ",
        reply: REPLY_OK,

    client:
        request: {
            method: POST,
            url: "http://{addr}/chunks",
            body: (Body::wrap_stream(Body::from("foo bar baz"))),
        },
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
        request: {
            method: POST,
            url: "http://{addr}/empty",
            headers: {
                "Content-Length" => "0",
            },
        },
        response:
            status: OK,
            headers: {},
            body: None,
}

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
        request: {
            method: HEAD,
            url: "http://{addr}/head",
        },
        response:
            status: OK,
            headers: {},
            body: None,
}

test! {
    name: client_response_transfer_encoding_not_chunked,

    server:
        expected: "\
            GET /te-not-chunked HTTP/1.1\r\n\
            host: {addr}\r\n\
            \r\n\
            ",
        reply: "\
            HTTP/1.1 200 OK\r\n\
            transfer-encoding: yolo\r\n\
            \r\n\
            hallo\
            ",

    client:
        request: {
            method: GET,
            url: "http://{addr}/te-not-chunked",
        },
        response:
            status: OK,
            headers: {
                "transfer-encoding" => "yolo",
            },
            body: &b"hallo"[..],
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
        request: {
            method: GET,
            url: "http://{addr}/pipe",
        },
        response:
            status: OK,
            headers: {},
            body: None,
}

test! {
    name: client_requires_absolute_uri,

    server:
        expected: "won't get here {addr}",
        reply: "won't reply",

    client:
        request: {
            method: GET,
            url: "/relative-{addr}",
        },
        error: |err| err.to_string() == "client requires absolute-form URIs",
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
        request: {
            method: GET,
            url: "http://{addr}/err",
        },
        error: |err| err.is_incomplete_message(),
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
        request: {
            method: GET,
            url: "http://{addr}/err",
        },
        // should get a Parse(Version) error
        error: |err| err.is_parse(),

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
        request: {
            method: POST,
            url: "http://{addr}/continue",
            headers: {
                "Content-Length" => "7",
            },
            body: "foo bar",
        },
        response:
            status: OK,
            headers: {},
            body: None,
}

test! {
    name: client_connect_method,

    server:
        expected: "\
            CONNECT {addr} HTTP/1.1\r\n\
            host: {addr}\r\n\
            \r\n\
            ",
        reply: "\
            HTTP/1.1 200 OK\r\n\
            \r\n\
            ",

    client:
        request: {
            method: CONNECT,
            url: "{addr}",
        },
        response:
            status: OK,
            headers: {},
            body: None,

}

test! {
    name: client_connect_method_with_absolute_uri,

    server:
        expected: "\
            CONNECT {addr} HTTP/1.1\r\n\
            host: {addr}\r\n\
            \r\n\
            ",
        reply: "\
            HTTP/1.1 200 OK\r\n\
            \r\n\
            ",

    client:
        request: {
            method: CONNECT,
            url: "http://{addr}",
        },
        response:
            status: OK,
            headers: {},
            body: None,

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
        request: {
            method: GET,
            url: "http://{addr}/no-host/{addr}",
        },
        response:
            status: OK,
            headers: {},
            body: None,
}

test! {
    name: client_set_http1_title_case_headers,

    server:
        expected: "\
            GET / HTTP/1.1\r\n\
            X-Test-Header: test\r\n\
            Host: {addr}\r\n\
            \r\n\
            ",
        reply: "\
            HTTP/1.1 200 OK\r\n\
            Content-Length: 0\r\n\
            \r\n\
            ",

    client:
        set_host: true,
        title_case_headers: true,
        request: {
            method: GET,
            url: "http://{addr}/",
            headers: {
                "X-Test-Header" => "test",
            },
        },
        response:
            status: OK,
            headers: {},
            body: None,
}

test! {
    name: client_h1_rejects_http2,

    server:
        expected: "won't get here {addr}",
        reply: "won't reply",

    client:
        request: {
            method: GET,
            url: "http://{addr}/",
            version: HTTP_2,
        },
        error: |err| err.to_string() == "request has unsupported HTTP version",
}

test! {
    name: client_always_rejects_http09,

    server:
        expected: "won't get here {addr}",
        reply: "won't reply",

    client:
        request: {
            method: GET,
            url: "http://{addr}/",
            version: HTTP_09,
        },
        error: |err| err.to_string() == "request has unsupported HTTP version",
}

mod dispatch_impl {
    use super::*;
    use std::io::{self, Read, Write};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    use futures_channel::{mpsc, oneshot};
    use futures_core::{self, Future};
    use futures_util::future::{FutureExt, TryFutureExt};
    use futures_util::stream::StreamExt;
    use http::Uri;
    use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
    use tokio::net::TcpStream;

    use super::support;
    use hyper::body::HttpBody;
    use hyper::client::connect::{Connected, Connection, HttpConnector};
    use hyper::Client;

    #[test]
    fn drop_body_before_eof_closes_connection() {
        // https://github.com/hyperium/hyper/issues/1353
        let _ = pretty_env_logger::try_init();

        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = server.local_addr().unwrap();
        let rt = support::runtime();
        let (closes_tx, closes) = mpsc::channel(10);
        let client = Client::builder().build(DebugConnector::with_http_and_closes(
            HttpConnector::new(),
            closes_tx,
        ));

        let (tx1, rx1) = oneshot::channel();

        thread::spawn(move || {
            let mut sock = server.accept().unwrap().0;
            sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            sock.set_write_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut buf = [0; 4096];
            sock.read(&mut buf).expect("read 1");
            let body = vec![b'x'; 1024 * 128];
            write!(
                sock,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n",
                body.len()
            )
            .expect("write head");
            let _ = sock.write_all(&body);
            let _ = tx1.send(());
        });

        let req = Request::builder()
            .uri(&*format!("http://{}/a", addr))
            .body(Body::empty())
            .unwrap();
        let res = client.request(req).map_ok(move |res| {
            assert_eq!(res.status(), hyper::StatusCode::OK);
        });
        let rx = rx1.expect("thread panicked");
        rt.block_on(async move {
            let (res, ()) = future::join(res, rx).await;
            res.unwrap();
            tokio::time::sleep(Duration::from_secs(1)).await;
        });

        rt.block_on(closes.into_future()).0.expect("closes");
    }

    #[test]
    fn dropped_client_closes_connection() {
        // https://github.com/hyperium/hyper/issues/1353
        let _ = pretty_env_logger::try_init();

        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = server.local_addr().unwrap();
        let rt = support::runtime();
        let (closes_tx, closes) = mpsc::channel(10);

        let (tx1, rx1) = oneshot::channel();

        thread::spawn(move || {
            let mut sock = server.accept().unwrap().0;
            sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            sock.set_write_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut buf = [0; 4096];
            sock.read(&mut buf).expect("read 1");
            let body = [b'x'; 64];
            write!(
                sock,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n",
                body.len()
            )
            .expect("write head");
            let _ = sock.write_all(&body);
            let _ = tx1.send(());
        });

        let res = {
            let client = Client::builder().build(DebugConnector::with_http_and_closes(
                HttpConnector::new(),
                closes_tx,
            ));

            let req = Request::builder()
                .uri(&*format!("http://{}/a", addr))
                .body(Body::empty())
                .unwrap();
            client
                .request(req)
                .and_then(move |res| {
                    assert_eq!(res.status(), hyper::StatusCode::OK);
                    concat(res)
                })
                .map_ok(|_| ())
        };
        // client is dropped
        let rx = rx1.expect("thread panicked");
        rt.block_on(async move {
            let (res, ()) = future::join(res, rx).await;
            res.unwrap();
            tokio::time::sleep(Duration::from_secs(1)).await;
        });

        rt.block_on(closes.into_future()).0.expect("closes");
    }

    #[tokio::test]
    async fn drop_client_closes_idle_connections() {
        use futures_util::future;

        let _ = pretty_env_logger::try_init();

        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = server.local_addr().unwrap();
        let (closes_tx, mut closes) = mpsc::channel(10);

        let (tx1, rx1) = oneshot::channel();
        let (_client_drop_tx, client_drop_rx) = oneshot::channel::<()>();

        thread::spawn(move || {
            let mut sock = server.accept().unwrap().0;
            sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            sock.set_write_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut buf = [0; 4096];
            sock.read(&mut buf).expect("read 1");
            let body = [b'x'; 64];
            write!(
                sock,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n",
                body.len()
            )
            .expect("write head");
            let _ = sock.write_all(&body);
            let _ = tx1.send(());

            // prevent this thread from closing until end of test, so the connection
            // stays open and idle until Client is dropped
            support::runtime().block_on(client_drop_rx.into_future())
        });

        let client = Client::builder().build(DebugConnector::with_http_and_closes(
            HttpConnector::new(),
            closes_tx,
        ));

        let req = Request::builder()
            .uri(&*format!("http://{}/a", addr))
            .body(Body::empty())
            .unwrap();
        let res = client.request(req).and_then(move |res| {
            assert_eq!(res.status(), hyper::StatusCode::OK);
            concat(res)
        });
        let rx = rx1.expect("thread panicked");

        let (res, ()) = future::join(res, rx).await;
        res.unwrap();

        // not closed yet, just idle
        future::poll_fn(|ctx| {
            assert!(Pin::new(&mut closes).poll_next(ctx).is_pending());
            Poll::Ready(())
        })
        .await;

        // drop to start the connections closing
        drop(client);

        // and wait a few ticks for the connections to close
        let t = tokio::time::sleep(Duration::from_millis(100)).map(|_| panic!("time out"));
        futures_util::pin_mut!(t);
        let close = closes.into_future().map(|(opt, _)| opt.expect("closes"));
        future::select(t, close).await;
    }

    #[tokio::test]
    async fn drop_response_future_closes_in_progress_connection() {
        let _ = pretty_env_logger::try_init();

        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = server.local_addr().unwrap();
        let (closes_tx, closes) = mpsc::channel(10);

        let (tx1, rx1) = oneshot::channel();
        let (_client_drop_tx, client_drop_rx) = std::sync::mpsc::channel::<()>();

        thread::spawn(move || {
            let mut sock = server.accept().unwrap().0;
            sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            sock.set_write_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut buf = [0; 4096];
            sock.read(&mut buf).expect("read 1");
            // we never write a response head
            // simulates a slow server operation
            let _ = tx1.send(());

            // prevent this thread from closing until end of test, so the connection
            // stays open and idle until Client is dropped
            let _ = client_drop_rx.recv();
        });

        let res = {
            let client = Client::builder().build(DebugConnector::with_http_and_closes(
                HttpConnector::new(),
                closes_tx,
            ));

            let req = Request::builder()
                .uri(&*format!("http://{}/a", addr))
                .body(Body::empty())
                .unwrap();
            client.request(req).map(|_| unreachable!())
        };

        future::select(res, rx1).await;

        // res now dropped
        let t = tokio::time::sleep(Duration::from_millis(100)).map(|_| panic!("time out"));
        futures_util::pin_mut!(t);
        let close = closes.into_future().map(|(opt, _)| opt.expect("closes"));
        future::select(t, close).await;
    }

    #[tokio::test]
    async fn drop_response_body_closes_in_progress_connection() {
        let _ = pretty_env_logger::try_init();

        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = server.local_addr().unwrap();
        let (closes_tx, closes) = mpsc::channel(10);

        let (tx1, rx1) = oneshot::channel();
        let (_client_drop_tx, client_drop_rx) = std::sync::mpsc::channel::<()>();

        thread::spawn(move || {
            let mut sock = server.accept().unwrap().0;
            sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            sock.set_write_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut buf = [0; 4096];
            sock.read(&mut buf).expect("read 1");
            write!(
                sock,
                "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n"
            )
            .expect("write head");
            let _ = tx1.send(());

            // prevent this thread from closing until end of test, so the connection
            // stays open and idle until Client is dropped
            let _ = client_drop_rx.recv();
        });

        let rx = rx1.expect("thread panicked");
        let res = {
            let client = Client::builder().build(DebugConnector::with_http_and_closes(
                HttpConnector::new(),
                closes_tx,
            ));

            let req = Request::builder()
                .uri(&*format!("http://{}/a", addr))
                .body(Body::empty())
                .unwrap();
            // notably, haven't read body yet
            client.request(req)
        };

        let (res, ()) = future::join(res, rx).await;
        // drop the body
        res.unwrap();

        // and wait a few ticks to see the connection drop
        let t = tokio::time::sleep(Duration::from_millis(100)).map(|_| panic!("time out"));
        futures_util::pin_mut!(t);
        let close = closes.into_future().map(|(opt, _)| opt.expect("closes"));
        future::select(t, close).await;
    }

    #[tokio::test]
    async fn no_keep_alive_closes_connection() {
        // https://github.com/hyperium/hyper/issues/1383
        let _ = pretty_env_logger::try_init();

        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = server.local_addr().unwrap();
        let (closes_tx, closes) = mpsc::channel(10);

        let (tx1, rx1) = oneshot::channel();
        let (_tx2, rx2) = std::sync::mpsc::channel::<()>();

        thread::spawn(move || {
            let mut sock = server.accept().unwrap().0;
            sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            sock.set_write_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut buf = [0; 4096];
            sock.read(&mut buf).expect("read 1");
            sock.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
                .unwrap();
            let _ = tx1.send(());

            // prevent this thread from closing until end of test, so the connection
            // stays open and idle until Client is dropped
            let _ = rx2.recv();
        });

        let client = Client::builder().pool_max_idle_per_host(0).build(
            DebugConnector::with_http_and_closes(HttpConnector::new(), closes_tx),
        );

        let req = Request::builder()
            .uri(&*format!("http://{}/a", addr))
            .body(Body::empty())
            .unwrap();
        let res = client.request(req).and_then(move |res| {
            assert_eq!(res.status(), hyper::StatusCode::OK);
            concat(res)
        });
        let rx = rx1.expect("thread panicked");

        let (res, ()) = future::join(res, rx).await;
        res.unwrap();

        let t = tokio::time::sleep(Duration::from_millis(100)).map(|_| panic!("time out"));
        futures_util::pin_mut!(t);
        let close = closes.into_future().map(|(opt, _)| opt.expect("closes"));
        future::select(t, close).await;
    }

    #[tokio::test]
    async fn socket_disconnect_closes_idle_conn() {
        // notably when keep-alive is enabled
        let _ = pretty_env_logger::try_init();

        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = server.local_addr().unwrap();
        let (closes_tx, closes) = mpsc::channel(10);

        let (tx1, rx1) = oneshot::channel();

        thread::spawn(move || {
            let mut sock = server.accept().unwrap().0;
            sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            sock.set_write_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut buf = [0; 4096];
            sock.read(&mut buf).expect("read 1");
            sock.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
                .unwrap();
            let _ = tx1.send(());
        });

        let client = Client::builder().build(DebugConnector::with_http_and_closes(
            HttpConnector::new(),
            closes_tx,
        ));

        let req = Request::builder()
            .uri(&*format!("http://{}/a", addr))
            .body(Body::empty())
            .unwrap();
        let res = client.request(req).and_then(move |res| {
            assert_eq!(res.status(), hyper::StatusCode::OK);
            concat(res)
        });
        let rx = rx1.expect("thread panicked");

        let (res, ()) = future::join(res, rx).await;
        res.unwrap();

        let t = tokio::time::sleep(Duration::from_millis(100)).map(|_| panic!("time out"));
        futures_util::pin_mut!(t);
        let close = closes.into_future().map(|(opt, _)| opt.expect("closes"));
        future::select(t, close).await;
    }

    #[test]
    fn connect_call_is_lazy() {
        // We especially don't want connects() triggered if there's
        // idle connections that the Checkout would have found
        let _ = pretty_env_logger::try_init();

        let _rt = support::runtime();
        let connector = DebugConnector::new();
        let connects = connector.connects.clone();

        let client = Client::builder().build(connector);

        assert_eq!(connects.load(Ordering::Relaxed), 0);
        let req = Request::builder()
            .uri("http://hyper.local/a")
            .body(Body::empty())
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
        let rt = support::runtime();
        let connector = DebugConnector::new();
        let connects = connector.connects.clone();

        let client = Client::builder().build(connector);

        let (tx1, rx1) = oneshot::channel();
        let (tx2, rx2) = oneshot::channel();
        thread::spawn(move || {
            let mut sock = server.accept().unwrap().0;
            //drop(server);
            sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            sock.set_write_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut buf = [0; 4096];
            sock.read(&mut buf).expect("read 1");
            sock.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
                .expect("write 1");
            let _ = tx1.send(());

            let n2 = sock.read(&mut buf).expect("read 2");
            assert_ne!(n2, 0);
            let second_get = "GET /b HTTP/1.1\r\n";
            assert_eq!(s(&buf[..second_get.len()]), second_get);
            sock.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
                .expect("write 2");
            let _ = tx2.send(());
        });

        assert_eq!(connects.load(Ordering::SeqCst), 0);

        let rx = rx1.expect("thread panicked");
        let req = Request::builder()
            .uri(&*format!("http://{}/a", addr))
            .body(Body::empty())
            .unwrap();
        let res = client.request(req);
        rt.block_on(future::join(res, rx).map(|r| r.0)).unwrap();

        assert_eq!(connects.load(Ordering::SeqCst), 1);

        // sleep real quick to let the threadpool put connection in ready
        // state and back into client pool
        thread::sleep(Duration::from_millis(50));

        let rx = rx2.expect("thread panicked");
        let req = Request::builder()
            .uri(&*format!("http://{}/b", addr))
            .body(Body::empty())
            .unwrap();
        let res = client.request(req);
        rt.block_on(future::join(res, rx).map(|r| r.0)).unwrap();

        assert_eq!(
            connects.load(Ordering::SeqCst),
            1,
            "second request should still only have 1 connect"
        );
        drop(client);
    }

    #[test]
    fn client_keep_alive_extra_body() {
        let _ = pretty_env_logger::try_init();
        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = server.local_addr().unwrap();
        let rt = support::runtime();

        let connector = DebugConnector::new();
        let connects = connector.connects.clone();

        let client = Client::builder().build(connector);

        let (tx1, rx1) = oneshot::channel();
        let (tx2, rx2) = oneshot::channel();
        thread::spawn(move || {
            let mut sock = server.accept().unwrap().0;
            sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            sock.set_write_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut buf = [0; 4096];
            sock.read(&mut buf).expect("read 1");
            sock.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nhello")
                .expect("write 1");
            // the body "hello", while ignored because its a HEAD request, should mean the connection
            // cannot be put back in the pool
            let _ = tx1.send(());

            let mut sock2 = server.accept().unwrap().0;
            let n2 = sock2.read(&mut buf).expect("read 2");
            assert_ne!(n2, 0);
            let second_get = "GET /b HTTP/1.1\r\n";
            assert_eq!(s(&buf[..second_get.len()]), second_get);
            sock2
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
                .expect("write 2");
            let _ = tx2.send(());
        });

        assert_eq!(connects.load(Ordering::Relaxed), 0);

        let rx = rx1.expect("thread panicked");
        let req = Request::builder()
            .method("HEAD")
            .uri(&*format!("http://{}/a", addr))
            .body(Body::empty())
            .unwrap();
        let res = client.request(req);
        rt.block_on(future::join(res, rx).map(|r| r.0)).unwrap();

        assert_eq!(connects.load(Ordering::Relaxed), 1);

        let rx = rx2.expect("thread panicked");
        let req = Request::builder()
            .uri(&*format!("http://{}/b", addr))
            .body(Body::empty())
            .unwrap();
        let res = client.request(req);
        rt.block_on(future::join(res, rx).map(|r| r.0)).unwrap();

        assert_eq!(connects.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn client_keep_alive_when_response_before_request_body_ends() {
        let _ = pretty_env_logger::try_init();
        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = server.local_addr().unwrap();
        let rt = support::runtime();

        let connector = DebugConnector::new();
        let connects = connector.connects.clone();

        let client = Client::builder().build(connector);

        let (tx1, rx1) = oneshot::channel();
        let (tx2, rx2) = oneshot::channel();
        let (tx3, rx3) = oneshot::channel();
        thread::spawn(move || {
            let mut sock = server.accept().unwrap().0;
            sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            sock.set_write_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut buf = [0; 4096];
            sock.read(&mut buf).expect("read 1");
            sock.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
                .expect("write 1");
            // after writing the response, THEN stream the body
            let _ = tx1.send(());

            sock.read(&mut buf).expect("read 2");
            let _ = tx2.send(());

            let n2 = sock.read(&mut buf).expect("read 3");
            assert_ne!(n2, 0);
            let second_get = "GET /b HTTP/1.1\r\n";
            assert_eq!(s(&buf[..second_get.len()]), second_get);
            sock.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
                .expect("write 2");
            let _ = tx3.send(());
        });

        assert_eq!(connects.load(Ordering::Relaxed), 0);

        let delayed_body = rx1
            .then(|_| tokio::time::sleep(Duration::from_millis(200)))
            .map(|_| Ok::<_, ()>("hello a"))
            .map_err(|_| -> hyper::Error { panic!("rx1") })
            .into_stream();

        let rx = rx2.expect("thread panicked");
        let req = Request::builder()
            .method("POST")
            .uri(&*format!("http://{}/a", addr))
            .body(Body::wrap_stream(delayed_body))
            .unwrap();
        let client2 = client.clone();

        // req 1
        let fut = future::join(client.request(req), rx)
            .then(|_| tokio::time::sleep(Duration::from_millis(200)))
            // req 2
            .then(move |()| {
                let rx = rx3.expect("thread panicked");
                let req = Request::builder()
                    .uri(&*format!("http://{}/b", addr))
                    .body(Body::empty())
                    .unwrap();
                future::join(client2.request(req), rx).map(|r| r.0)
            });

        rt.block_on(fut).unwrap();

        assert_eq!(connects.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn client_keep_alive_eager_when_chunked() {
        // If a response body has been read to completion, with completion
        // determined by some other factor, like decompression, and thus
        // it is in't polled a final time to clear the final 0-len chunk,
        // try to eagerly clear it so the connection can still be used.

        let _ = pretty_env_logger::try_init();
        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = server.local_addr().unwrap();
        let connector = DebugConnector::new();
        let connects = connector.connects.clone();

        let client = Client::builder().build(connector);

        let (tx1, rx1) = oneshot::channel();
        let (tx2, rx2) = oneshot::channel();
        thread::spawn(move || {
            let mut sock = server.accept().unwrap().0;
            //drop(server);
            sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            sock.set_write_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut buf = [0; 4096];
            sock.read(&mut buf).expect("read 1");
            sock.write_all(
                b"\
                HTTP/1.1 200 OK\r\n\
                transfer-encoding: chunked\r\n\
                \r\n\
                5\r\n\
                hello\r\n\
                0\r\n\r\n\
            ",
            )
            .expect("write 1");
            let _ = tx1.send(());

            let n2 = sock.read(&mut buf).expect("read 2");
            assert_ne!(n2, 0, "bytes of second request");
            let second_get = "GET /b HTTP/1.1\r\n";
            assert_eq!(s(&buf[..second_get.len()]), second_get);
            sock.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
                .expect("write 2");
            let _ = tx2.send(());
        });

        assert_eq!(connects.load(Ordering::SeqCst), 0);

        let rx = rx1.expect("thread panicked");
        let req = Request::builder()
            .uri(&*format!("http://{}/a", addr))
            .body(Body::empty())
            .unwrap();
        let fut = client.request(req);

        let mut resp = future::join(fut, rx).map(|r| r.0).await.unwrap();
        assert_eq!(connects.load(Ordering::SeqCst), 1);
        assert_eq!(resp.status(), 200);
        assert_eq!(resp.headers()["transfer-encoding"], "chunked");

        // Read the "hello" chunk...
        let chunk = resp.body_mut().data().await.unwrap().unwrap();
        assert_eq!(chunk, "hello");

        // With our prior knowledge, we know that's the end of the body.
        // So just drop the body, without polling for the `0\r\n\r\n` end.
        drop(resp);

        // sleep real quick to let the threadpool put connection in ready
        // state and back into client pool
        tokio::time::sleep(Duration::from_millis(50)).await;

        let rx = rx2.expect("thread panicked");
        let req = Request::builder()
            .uri(&*format!("http://{}/b", addr))
            .body(Body::empty())
            .unwrap();
        let fut = client.request(req);
        future::join(fut, rx).map(|r| r.0).await.unwrap();

        assert_eq!(
            connects.load(Ordering::SeqCst),
            1,
            "second request should still only have 1 connect"
        );
        drop(client);
    }

    #[test]
    fn connect_proxy_sends_absolute_uri() {
        let _ = pretty_env_logger::try_init();
        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = server.local_addr().unwrap();
        let rt = support::runtime();
        let connector = DebugConnector::new().proxy();

        let client = Client::builder().build(connector);

        let (tx1, rx1) = oneshot::channel();
        thread::spawn(move || {
            let mut sock = server.accept().unwrap().0;
            //drop(server);
            sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            sock.set_write_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut buf = [0; 4096];
            let n = sock.read(&mut buf).expect("read 1");
            let expected = format!(
                "GET http://{addr}/foo/bar HTTP/1.1\r\nhost: {addr}\r\n\r\n",
                addr = addr
            );
            assert_eq!(s(&buf[..n]), expected);

            sock.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
                .expect("write 1");
            let _ = tx1.send(());
        });

        let rx = rx1.expect("thread panicked");
        let req = Request::builder()
            .uri(&*format!("http://{}/foo/bar", addr))
            .body(Body::empty())
            .unwrap();
        let res = client.request(req);
        rt.block_on(future::join(res, rx).map(|r| r.0)).unwrap();
    }

    #[test]
    fn connect_proxy_http_connect_sends_authority_form() {
        let _ = pretty_env_logger::try_init();
        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = server.local_addr().unwrap();
        let rt = support::runtime();
        let connector = DebugConnector::new().proxy();

        let client = Client::builder().build(connector);

        let (tx1, rx1) = oneshot::channel();
        thread::spawn(move || {
            let mut sock = server.accept().unwrap().0;
            //drop(server);
            sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            sock.set_write_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut buf = [0; 4096];
            let n = sock.read(&mut buf).expect("read 1");
            let expected = format!(
                "CONNECT {addr} HTTP/1.1\r\nhost: {addr}\r\n\r\n",
                addr = addr
            );
            assert_eq!(s(&buf[..n]), expected);

            sock.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
                .expect("write 1");
            let _ = tx1.send(());
        });

        let rx = rx1.expect("thread panicked");
        let req = Request::builder()
            .method("CONNECT")
            .uri(&*format!("http://{}/useless/path", addr))
            .body(Body::empty())
            .unwrap();
        let res = client.request(req);
        rt.block_on(future::join(res, rx).map(|r| r.0)).unwrap();
    }

    #[test]
    fn client_upgrade() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let _ = pretty_env_logger::try_init();
        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = server.local_addr().unwrap();
        let rt = support::runtime();

        let connector = DebugConnector::new();

        let client = Client::builder().build(connector);

        let (tx1, rx1) = oneshot::channel();
        thread::spawn(move || {
            let mut sock = server.accept().unwrap().0;
            sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            sock.set_write_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut buf = [0; 4096];
            sock.read(&mut buf).expect("read 1");
            sock.write_all(
                b"\
                HTTP/1.1 101 Switching Protocols\r\n\
                Upgrade: foobar\r\n\
                \r\n\
                foobar=ready\
            ",
            )
            .unwrap();
            let _ = tx1.send(());

            let n = sock.read(&mut buf).expect("read 2");
            assert_eq!(&buf[..n], b"foo=bar");
            sock.write_all(b"bar=foo").expect("write 2");
        });

        let rx = rx1.expect("thread panicked");

        let req = Request::builder()
            .method("GET")
            .uri(&*format!("http://{}/up", addr))
            .body(Body::empty())
            .unwrap();

        let res = client.request(req);
        let res = rt.block_on(future::join(res, rx).map(|r| r.0)).unwrap();

        assert_eq!(res.status(), 101);
        let upgraded = rt.block_on(hyper::upgrade::on(res)).expect("on_upgrade");

        let parts = upgraded.downcast::<DebugStream>().unwrap();
        assert_eq!(s(&parts.read_buf), "foobar=ready");

        let mut io = parts.io;
        rt.block_on(io.write_all(b"foo=bar")).unwrap();
        let mut vec = vec![];
        rt.block_on(io.read_to_end(&mut vec)).unwrap();
        assert_eq!(vec, b"bar=foo");
    }

    #[test]
    fn alpn_h2() {
        use hyper::server::conn::Http;
        use hyper::service::service_fn;
        use hyper::Response;
        use tokio::net::TcpListener;

        let _ = pretty_env_logger::try_init();
        let rt = support::runtime();
        let listener = rt
            .block_on(TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0))))
            .unwrap();
        let addr = listener.local_addr().unwrap();
        let mut connector = DebugConnector::new();
        connector.alpn_h2 = true;
        let connects = connector.connects.clone();

        let client = Client::builder().build::<_, ::hyper::Body>(connector);

        rt.spawn(async move {
            let (socket, _addr) = listener.accept().await.expect("accept");
            Http::new()
                .http2_only(true)
                .serve_connection(
                    socket,
                    service_fn(|req| async move {
                        assert_eq!(req.headers().get("host"), None);
                        Ok::<_, hyper::Error>(Response::new(Body::empty()))
                    }),
                )
                .await
                .expect("server");
        });

        assert_eq!(connects.load(Ordering::SeqCst), 0);

        let url = format!("http://{}/a", addr)
            .parse::<::hyper::Uri>()
            .unwrap();
        let res1 = client.get(url.clone());
        let res2 = client.get(url.clone());
        let res3 = client.get(url.clone());
        rt.block_on(future::try_join3(res1, res2, res3)).unwrap();

        // Since the client doesn't know it can ALPN at first, it will have
        // started 3 connections. But, the server above will only handle 1,
        // so the unwrapped responses futures show it still worked.
        assert_eq!(connects.load(Ordering::SeqCst), 3);

        let res4 = client.get(url);
        rt.block_on(res4).unwrap();

        assert_eq!(
            connects.load(Ordering::SeqCst),
            3,
            "after ALPN, no more connects"
        );
        drop(client);
    }

    #[derive(Clone)]
    struct DebugConnector {
        http: HttpConnector,
        closes: mpsc::Sender<()>,
        connects: Arc<AtomicUsize>,
        is_proxy: bool,
        alpn_h2: bool,
    }

    impl DebugConnector {
        fn new() -> DebugConnector {
            let http = HttpConnector::new();
            let (tx, _) = mpsc::channel(10);
            DebugConnector::with_http_and_closes(http, tx)
        }

        fn with_http_and_closes(http: HttpConnector, closes: mpsc::Sender<()>) -> DebugConnector {
            DebugConnector {
                http,
                closes,
                connects: Arc::new(AtomicUsize::new(0)),
                is_proxy: false,
                alpn_h2: false,
            }
        }

        fn proxy(mut self) -> Self {
            self.is_proxy = true;
            self
        }
    }

    impl hyper::service::Service<Uri> for DebugConnector {
        type Response = DebugStream;
        type Error = <HttpConnector as hyper::service::Service<Uri>>::Error;
        type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

        fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            // don't forget to check inner service is ready :)
            hyper::service::Service::<Uri>::poll_ready(&mut self.http, cx)
        }

        fn call(&mut self, dst: Uri) -> Self::Future {
            self.connects.fetch_add(1, Ordering::SeqCst);
            let closes = self.closes.clone();
            let is_proxy = self.is_proxy;
            let is_alpn_h2 = self.alpn_h2;
            Box::pin(self.http.call(dst).map_ok(move |tcp| DebugStream {
                tcp,
                on_drop: closes,
                is_alpn_h2,
                is_proxy,
            }))
        }
    }

    struct DebugStream {
        tcp: TcpStream,
        on_drop: mpsc::Sender<()>,
        is_alpn_h2: bool,
        is_proxy: bool,
    }

    impl Drop for DebugStream {
        fn drop(&mut self) {
            let _ = self.on_drop.try_send(());
        }
    }

    impl AsyncWrite for DebugStream {
        fn poll_shutdown(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
        ) -> Poll<Result<(), io::Error>> {
            Pin::new(&mut self.tcp).poll_shutdown(cx)
        }

        fn poll_flush(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
        ) -> Poll<Result<(), io::Error>> {
            Pin::new(&mut self.tcp).poll_flush(cx)
        }

        fn poll_write(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<Result<usize, io::Error>> {
            Pin::new(&mut self.tcp).poll_write(cx, buf)
        }
    }

    impl AsyncRead for DebugStream {
        fn poll_read(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &mut ReadBuf<'_>,
        ) -> Poll<io::Result<()>> {
            Pin::new(&mut self.tcp).poll_read(cx, buf)
        }
    }

    impl Connection for DebugStream {
        fn connected(&self) -> Connected {
            let connected = self.tcp.connected().proxy(self.is_proxy);

            if self.is_alpn_h2 {
                connected.negotiated_h2()
            } else {
                connected
            }
        }
    }
}

mod conn {
    use std::io::{self, Read, Write};
    use std::net::{SocketAddr, TcpListener};
    use std::pin::Pin;
    use std::task::{Context, Poll};
    use std::thread;
    use std::time::Duration;

    use futures_channel::oneshot;
    use futures_util::future::{self, poll_fn, FutureExt, TryFutureExt};
    use futures_util::StreamExt;
    use tokio::io::{AsyncRead, AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _, ReadBuf};
    use tokio::net::{TcpListener as TkTcpListener, TcpStream};

    use hyper::client::conn;
    use hyper::{self, Body, Method, Request};

    use super::{concat, s, support, tcp_connect, FutureHyperExt};

    #[tokio::test]
    async fn get() {
        let _ = ::pretty_env_logger::try_init();
        let listener = TkTcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
            .await
            .unwrap();
        let addr = listener.local_addr().unwrap();

        let server = async move {
            let mut sock = listener.accept().await.unwrap().0;
            let mut buf = [0; 4096];
            let n = sock.read(&mut buf).await.expect("read 1");

            // Notably:
            // - Just a path, since just a path was set
            // - No host, since no host was set
            let expected = "GET /a HTTP/1.1\r\n\r\n";
            assert_eq!(s(&buf[..n]), expected);

            sock.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
                .await
                .unwrap();
        };

        let client = async move {
            let tcp = tcp_connect(&addr).await.expect("connect");
            let (mut client, conn) = conn::handshake(tcp).await.expect("handshake");

            tokio::task::spawn(async move {
                conn.await.expect("http conn");
            });

            let req = Request::builder()
                .uri("/a")
                .body(Default::default())
                .unwrap();
            let mut res = client.send_request(req).await.expect("send_request");
            assert_eq!(res.status(), hyper::StatusCode::OK);
            assert!(res.body_mut().next().await.is_none());
        };

        future::join(server, client).await;
    }

    #[test]
    fn incoming_content_length() {
        use hyper::body::HttpBody;

        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = server.local_addr().unwrap();
        let rt = support::runtime();

        let (tx1, rx1) = oneshot::channel();

        thread::spawn(move || {
            let mut sock = server.accept().unwrap().0;
            sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            sock.set_write_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut buf = [0; 4096];
            let n = sock.read(&mut buf).expect("read 1");

            let expected = "GET / HTTP/1.1\r\n\r\n";
            assert_eq!(s(&buf[..n]), expected);

            sock.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nhello")
                .unwrap();
            let _ = tx1.send(());
        });

        let tcp = rt.block_on(tcp_connect(&addr)).unwrap();

        let (mut client, conn) = rt.block_on(conn::handshake(tcp)).unwrap();

        rt.spawn(conn.map_err(|e| panic!("conn error: {}", e)).map(|_| ()));

        let req = Request::builder()
            .uri("/")
            .body(Default::default())
            .unwrap();
        let res = client.send_request(req).and_then(move |mut res| {
            assert_eq!(res.status(), hyper::StatusCode::OK);
            assert_eq!(res.body().size_hint().exact(), Some(5));
            assert!(!res.body().is_end_stream());
            poll_fn(move |ctx| Pin::new(res.body_mut()).poll_data(ctx)).map(Option::unwrap)
        });

        let rx = rx1.expect("thread panicked");
        let rx = rx.then(|_| tokio::time::sleep(Duration::from_millis(200)));
        let chunk = rt.block_on(future::join(res, rx).map(|r| r.0)).unwrap();
        assert_eq!(chunk.len(), 5);
    }

    #[test]
    fn aborted_body_isnt_completed() {
        let _ = ::pretty_env_logger::try_init();
        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = server.local_addr().unwrap();
        let rt = support::runtime();

        let (tx, rx) = oneshot::channel();
        let server = thread::spawn(move || {
            let mut sock = server.accept().unwrap().0;
            sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            sock.set_write_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let expected = "POST / HTTP/1.1\r\ntransfer-encoding: chunked\r\n\r\n5\r\nhello\r\n";
            let mut buf = vec![0; expected.len()];
            sock.read_exact(&mut buf).expect("read 1");
            assert_eq!(s(&buf), expected);

            let _ = tx.send(());

            assert_eq!(sock.read(&mut buf).expect("read 2"), 0);
        });

        let tcp = rt.block_on(tcp_connect(&addr)).unwrap();

        let (mut client, conn) = rt.block_on(conn::handshake(tcp)).unwrap();

        rt.spawn(conn.map_err(|e| panic!("conn error: {}", e)).map(|_| ()));

        let (mut sender, body) = Body::channel();
        let sender = thread::spawn(move || {
            sender.try_send_data("hello".into()).expect("try_send_data");
            support::runtime().block_on(rx).unwrap();
            sender.abort();
        });

        let req = Request::builder()
            .method(Method::POST)
            .uri("/")
            .body(body)
            .unwrap();
        let res = client.send_request(req);
        rt.block_on(res).unwrap_err();

        server.join().expect("server thread panicked");
        sender.join().expect("sender thread panicked");
    }

    #[test]
    fn uri_absolute_form() {
        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = server.local_addr().unwrap();
        let rt = support::runtime();

        let (tx1, rx1) = oneshot::channel();

        thread::spawn(move || {
            let mut sock = server.accept().unwrap().0;
            sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            sock.set_write_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut buf = [0; 4096];
            let n = sock.read(&mut buf).expect("read 1");

            // Notably:
            // - Still no Host header, since it wasn't set
            let expected = "GET http://hyper.local/a HTTP/1.1\r\n\r\n";
            assert_eq!(s(&buf[..n]), expected);

            sock.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
                .unwrap();
            let _ = tx1.send(());
        });

        let tcp = rt.block_on(tcp_connect(&addr)).unwrap();

        let (mut client, conn) = rt.block_on(conn::handshake(tcp)).unwrap();

        rt.spawn(conn.map_err(|e| panic!("conn error: {}", e)).map(|_| ()));

        let req = Request::builder()
            .uri("http://hyper.local/a")
            .body(Default::default())
            .unwrap();

        let res = client.send_request(req).and_then(move |res| {
            assert_eq!(res.status(), hyper::StatusCode::OK);
            concat(res)
        });
        let rx = rx1.expect("thread panicked");
        let rx = rx.then(|_| tokio::time::sleep(Duration::from_millis(200)));
        rt.block_on(future::join(res, rx).map(|r| r.0)).unwrap();
    }

    #[test]
    fn http1_conn_coerces_http2_request() {
        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = server.local_addr().unwrap();
        let rt = support::runtime();

        let (tx1, rx1) = oneshot::channel();

        thread::spawn(move || {
            let mut sock = server.accept().unwrap().0;
            sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            sock.set_write_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut buf = [0; 4096];
            let n = sock.read(&mut buf).expect("read 1");

            // Not HTTP/2, nor panicked
            let expected = "GET /a HTTP/1.1\r\n\r\n";
            assert_eq!(s(&buf[..n]), expected);

            sock.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
                .unwrap();
            let _ = tx1.send(());
        });

        let tcp = rt.block_on(tcp_connect(&addr)).unwrap();

        let (mut client, conn) = rt.block_on(conn::handshake(tcp)).unwrap();

        rt.spawn(conn.map_err(|e| panic!("conn error: {}", e)).map(|_| ()));

        let req = Request::builder()
            .uri("/a")
            .version(hyper::Version::HTTP_2)
            .body(Default::default())
            .unwrap();

        let res = client.send_request(req).and_then(move |res| {
            assert_eq!(res.status(), hyper::StatusCode::OK);
            concat(res)
        });
        let rx = rx1.expect("thread panicked");
        let rx = rx.then(|_| tokio::time::sleep(Duration::from_millis(200)));
        rt.block_on(future::join(res, rx).map(|r| r.0)).unwrap();
    }

    #[test]
    fn pipeline() {
        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = server.local_addr().unwrap();
        let rt = support::runtime();

        let (tx1, rx1) = oneshot::channel();

        thread::spawn(move || {
            let mut sock = server.accept().unwrap().0;
            sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            sock.set_write_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut buf = [0; 4096];
            sock.read(&mut buf).expect("read 1");
            sock.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
                .unwrap();

            let _ = tx1.send(Ok::<_, ()>(()));
        });

        let tcp = rt.block_on(tcp_connect(&addr)).unwrap();

        let (mut client, conn) = rt.block_on(conn::handshake(tcp)).unwrap();

        rt.spawn(conn.map_err(|e| panic!("conn error: {}", e)).map(|_| ()));

        let req = Request::builder()
            .uri("/a")
            .body(Default::default())
            .unwrap();
        let res1 = client.send_request(req).and_then(move |res| {
            assert_eq!(res.status(), hyper::StatusCode::OK);
            concat(res)
        });

        // pipelined request will hit NotReady, and thus should return an Error::Cancel
        let req = Request::builder()
            .uri("/b")
            .body(Default::default())
            .unwrap();
        let res2 = client.send_request(req).map(|result| {
            let err = result.expect_err("res2");
            assert!(err.is_canceled(), "err not canceled, {:?}", err);
            Ok::<_, ()>(())
        });

        let rx = rx1.expect("thread panicked");
        let rx = rx.then(|_| tokio::time::sleep(Duration::from_millis(200)));
        rt.block_on(future::join3(res1, res2, rx).map(|r| r.0))
            .unwrap();
    }

    #[test]
    fn upgrade() {
        let _ = ::pretty_env_logger::try_init();

        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = server.local_addr().unwrap();
        let rt = support::runtime();

        let (tx1, rx1) = oneshot::channel();

        thread::spawn(move || {
            let mut sock = server.accept().unwrap().0;
            sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            sock.set_write_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut buf = [0; 4096];
            sock.read(&mut buf).expect("read 1");
            sock.write_all(
                b"\
                HTTP/1.1 101 Switching Protocols\r\n\
                Upgrade: foobar\r\n\
                \r\n\
                foobar=ready\
            ",
            )
            .unwrap();
            let _ = tx1.send(());

            let n = sock.read(&mut buf).expect("read 2");
            assert_eq!(&buf[..n], b"foo=bar");
            sock.write_all(b"bar=foo").expect("write 2");
        });

        let tcp = rt.block_on(tcp_connect(&addr)).unwrap();

        let io = DebugStream {
            tcp,
            shutdown_called: false,
        };

        let (mut client, mut conn) = rt.block_on(conn::handshake(io)).unwrap();

        {
            let until_upgrade = poll_fn(|ctx| conn.poll_without_shutdown(ctx));

            let req = Request::builder()
                .uri("/a")
                .body(Default::default())
                .unwrap();
            let res = client.send_request(req).and_then(move |res| {
                assert_eq!(res.status(), hyper::StatusCode::SWITCHING_PROTOCOLS);
                assert_eq!(res.headers()["Upgrade"], "foobar");
                concat(res)
            });

            let rx = rx1.expect("thread panicked");
            let rx = rx.then(|_| tokio::time::sleep(Duration::from_millis(200)));
            rt.block_on(future::join3(until_upgrade, res, rx).map(|r| r.0))
                .unwrap();

            // should not be ready now
            rt.block_on(poll_fn(|ctx| {
                assert!(client.poll_ready(ctx).is_pending());
                Poll::Ready(Ok::<_, ()>(()))
            }))
            .unwrap();
        }

        let parts = conn.into_parts();
        let mut io = parts.io;
        let buf = parts.read_buf;

        assert_eq!(buf, b"foobar=ready"[..]);
        assert!(!io.shutdown_called, "upgrade shouldn't shutdown AsyncWrite");
        rt.block_on(poll_fn(|ctx| {
            let ready = client.poll_ready(ctx);
            assert_matches!(ready, Poll::Ready(Err(_)));
            ready
        }))
        .unwrap_err();

        let mut vec = vec![];
        rt.block_on(io.write_all(b"foo=bar")).unwrap();
        rt.block_on(io.read_to_end(&mut vec)).unwrap();
        assert_eq!(vec, b"bar=foo");
    }

    #[test]
    fn connect_method() {
        let _ = ::pretty_env_logger::try_init();

        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = server.local_addr().unwrap();
        let rt = support::runtime();

        let (tx1, rx1) = oneshot::channel();

        thread::spawn(move || {
            let mut sock = server.accept().unwrap().0;
            sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            sock.set_write_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut buf = [0; 4096];
            sock.read(&mut buf).expect("read 1");
            sock.write_all(
                b"\
                HTTP/1.1 200 OK\r\n\
                \r\n\
                foobar=ready\
            ",
            )
            .unwrap();
            let _ = tx1.send(Ok::<_, ()>(()));

            let n = sock.read(&mut buf).expect("read 2");
            assert_eq!(&buf[..n], b"foo=bar", "sock read 2 bytes");
            sock.write_all(b"bar=foo").expect("write 2");
        });

        let tcp = rt.block_on(tcp_connect(&addr)).unwrap();

        let io = DebugStream {
            tcp,
            shutdown_called: false,
        };

        let (mut client, mut conn) = rt.block_on(conn::handshake(io)).unwrap();

        {
            let until_tunneled = poll_fn(|ctx| conn.poll_without_shutdown(ctx));

            let req = Request::builder()
                .method("CONNECT")
                .uri(addr.to_string())
                .body(Default::default())
                .unwrap();
            let res = client
                .send_request(req)
                .and_then(move |res| {
                    assert_eq!(res.status(), hyper::StatusCode::OK);
                    concat(res)
                })
                .map_ok(|body| {
                    assert_eq!(body.as_ref(), b"");
                });

            let rx = rx1.expect("thread panicked");
            let rx = rx.then(|_| tokio::time::sleep(Duration::from_millis(200)));
            rt.block_on(future::join3(until_tunneled, res, rx).map(|r| r.0))
                .unwrap();

            // should not be ready now
            rt.block_on(poll_fn(|ctx| {
                assert!(client.poll_ready(ctx).is_pending());
                Poll::Ready(Ok::<_, ()>(()))
            }))
            .unwrap();
        }

        let parts = conn.into_parts();
        let mut io = parts.io;
        let buf = parts.read_buf;

        assert_eq!(buf, b"foobar=ready"[..]);
        assert!(!io.shutdown_called, "tunnel shouldn't shutdown AsyncWrite");

        rt.block_on(poll_fn(|ctx| {
            let ready = client.poll_ready(ctx);
            assert_matches!(ready, Poll::Ready(Err(_)));
            ready
        }))
        .unwrap_err();

        let mut vec = vec![];
        rt.block_on(io.write_all(b"foo=bar")).unwrap();
        rt.block_on(io.read_to_end(&mut vec)).unwrap();
        assert_eq!(vec, b"bar=foo");
    }

    #[tokio::test]
    async fn http2_detect_conn_eof() {
        use futures_util::future;
        use hyper::service::{make_service_fn, service_fn};
        use hyper::{Response, Server};

        let _ = pretty_env_logger::try_init();

        let server = Server::bind(&([127, 0, 0, 1], 0).into())
            .http2_only(true)
            .serve(make_service_fn(|_| async move {
                Ok::<_, hyper::Error>(service_fn(|_req| {
                    future::ok::<_, hyper::Error>(Response::new(Body::empty()))
                }))
            }));
        let addr = server.local_addr();
        let (shdn_tx, shdn_rx) = oneshot::channel();
        tokio::task::spawn(async move {
            server
                .with_graceful_shutdown(async move {
                    let _ = shdn_rx.await;
                })
                .await
                .expect("server")
        });

        let io = tcp_connect(&addr).await.expect("tcp connect");
        let (mut client, conn) = conn::Builder::new()
            .http2_only(true)
            .handshake::<_, Body>(io)
            .await
            .expect("http handshake");

        tokio::task::spawn(async move {
            conn.await.expect("client conn");
        });

        // Sanity check that client is ready
        future::poll_fn(|ctx| client.poll_ready(ctx))
            .await
            .expect("client poll ready sanity");

        let req = Request::builder()
            .uri(format!("http://{}/", addr))
            .body(Body::empty())
            .expect("request builder");

        client.send_request(req).await.expect("req1 send");

        // Sanity check that client is STILL ready
        future::poll_fn(|ctx| client.poll_ready(ctx))
            .await
            .expect("client poll ready after");

        // Trigger the server shutdown...
        let _ = shdn_tx.send(());

        // Allow time for graceful shutdown roundtrips...
        tokio::time::sleep(Duration::from_millis(100)).await;

        // After graceful shutdown roundtrips, the client should be closed...
        future::poll_fn(|ctx| client.poll_ready(ctx))
            .await
            .expect_err("client should be closed");
    }

    #[tokio::test]
    async fn http2_keep_alive_detects_unresponsive_server() {
        let _ = pretty_env_logger::try_init();

        let listener = TkTcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
            .await
            .unwrap();
        let addr = listener.local_addr().unwrap();

        // spawn a server that reads but doesn't write
        tokio::spawn(async move {
            let mut sock = listener.accept().await.unwrap().0;
            let mut buf = [0u8; 1024];
            loop {
                let n = sock.read(&mut buf).await.expect("server read");
                if n == 0 {
                    // server closed, lets go!
                    break;
                }
            }
        });

        let io = tcp_connect(&addr).await.expect("tcp connect");
        let (_client, conn) = conn::Builder::new()
            .http2_only(true)
            .http2_keep_alive_interval(Duration::from_secs(1))
            .http2_keep_alive_timeout(Duration::from_secs(1))
            // enable while idle since we aren't sending requests
            .http2_keep_alive_while_idle(true)
            .handshake::<_, Body>(io)
            .await
            .expect("http handshake");

        conn.await.expect_err("conn should time out");
    }

    #[tokio::test]
    async fn http2_keep_alive_not_while_idle() {
        // This tests that not setting `http2_keep_alive_while_idle(true)`
        // will use the default behavior which will NOT detect the server
        // is unresponsive while no streams are active.

        let _ = pretty_env_logger::try_init();

        let listener = TkTcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
            .await
            .unwrap();
        let addr = listener.local_addr().unwrap();

        // spawn a server that reads but doesn't write
        tokio::spawn(async move {
            let sock = listener.accept().await.unwrap().0;
            drain_til_eof(sock).await.expect("server read");
        });

        let io = tcp_connect(&addr).await.expect("tcp connect");
        let (mut client, conn) = conn::Builder::new()
            .http2_only(true)
            .http2_keep_alive_interval(Duration::from_secs(1))
            .http2_keep_alive_timeout(Duration::from_secs(1))
            .handshake::<_, Body>(io)
            .await
            .expect("http handshake");

        tokio::spawn(async move {
            conn.await.expect("client conn shouldn't error");
        });

        // sleep longer than keepalive would trigger
        tokio::time::sleep(Duration::from_secs(4)).await;

        future::poll_fn(|ctx| client.poll_ready(ctx))
            .await
            .expect("client should be open");
    }

    #[tokio::test]
    async fn http2_keep_alive_closes_open_streams() {
        let _ = pretty_env_logger::try_init();

        let listener = TkTcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
            .await
            .unwrap();
        let addr = listener.local_addr().unwrap();

        // spawn a server that reads but doesn't write
        tokio::spawn(async move {
            let sock = listener.accept().await.unwrap().0;
            drain_til_eof(sock).await.expect("server read");
        });

        let io = tcp_connect(&addr).await.expect("tcp connect");
        let (mut client, conn) = conn::Builder::new()
            .http2_only(true)
            .http2_keep_alive_interval(Duration::from_secs(1))
            .http2_keep_alive_timeout(Duration::from_secs(1))
            .handshake::<_, Body>(io)
            .await
            .expect("http handshake");

        tokio::spawn(async move {
            let err = conn.await.expect_err("client conn should timeout");
            assert!(err.is_timeout());
        });

        let req = http::Request::new(hyper::Body::empty());
        let err = client
            .send_request(req)
            .await
            .expect_err("request should timeout");
        assert!(err.is_timeout());

        let err = future::poll_fn(|ctx| client.poll_ready(ctx))
            .await
            .expect_err("client should be closed");
        assert!(
            err.is_closed(),
            "poll_ready error should be closed: {:?}",
            err
        );
    }

    #[tokio::test]
    async fn http2_keep_alive_with_responsive_server() {
        // Test that a responsive server works just when client keep
        // alive is enabled
        use hyper::service::service_fn;

        let _ = pretty_env_logger::try_init();

        let listener = TkTcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
            .await
            .unwrap();
        let addr = listener.local_addr().unwrap();

        // Spawn an HTTP2 server that reads the whole body and responds
        tokio::spawn(async move {
            let sock = listener.accept().await.unwrap().0;
            hyper::server::conn::Http::new()
                .http2_only(true)
                .serve_connection(
                    sock,
                    service_fn(|req| async move {
                        tokio::spawn(async move {
                            let _ = hyper::body::aggregate(req.into_body())
                                .await
                                .expect("server req body aggregate");
                        });
                        Ok::<_, hyper::Error>(http::Response::new(hyper::Body::empty()))
                    }),
                )
                .await
                .expect("serve_connection");
        });

        let io = tcp_connect(&addr).await.expect("tcp connect");
        let (mut client, conn) = conn::Builder::new()
            .http2_only(true)
            .http2_keep_alive_interval(Duration::from_secs(1))
            .http2_keep_alive_timeout(Duration::from_secs(1))
            .handshake::<_, Body>(io)
            .await
            .expect("http handshake");

        tokio::spawn(async move {
            conn.await.expect("client conn shouldn't error");
        });

        // Use a channel to keep request stream open
        let (_tx, body) = hyper::Body::channel();
        let req1 = http::Request::new(body);
        let _resp = client.send_request(req1).await.expect("send_request");

        // sleep longer than keepalive would trigger
        tokio::time::sleep(Duration::from_secs(4)).await;

        future::poll_fn(|ctx| client.poll_ready(ctx))
            .await
            .expect("client should be open");
    }

    async fn drain_til_eof<T: AsyncRead + Unpin>(mut sock: T) -> io::Result<()> {
        let mut buf = [0u8; 1024];
        loop {
            let n = sock.read(&mut buf).await?;
            if n == 0 {
                // socket closed, lets go!
                return Ok(());
            }
        }
    }

    struct DebugStream {
        tcp: TcpStream,
        shutdown_called: bool,
    }

    impl AsyncWrite for DebugStream {
        fn poll_shutdown(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
        ) -> Poll<Result<(), io::Error>> {
            self.shutdown_called = true;
            Pin::new(&mut self.tcp).poll_shutdown(cx)
        }

        fn poll_flush(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
        ) -> Poll<Result<(), io::Error>> {
            Pin::new(&mut self.tcp).poll_flush(cx)
        }

        fn poll_write(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<Result<usize, io::Error>> {
            Pin::new(&mut self.tcp).poll_write(cx, buf)
        }
    }

    impl AsyncRead for DebugStream {
        fn poll_read(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &mut ReadBuf<'_>,
        ) -> Poll<io::Result<()>> {
            Pin::new(&mut self.tcp).poll_read(cx, buf)
        }
    }
}

trait FutureHyperExt: TryFuture {
    fn expect(self, msg: &'static str) -> Pin<Box<dyn Future<Output = Self::Ok>>>;
}

impl<F> FutureHyperExt for F
where
    F: TryFuture + 'static,
    F::Error: std::fmt::Debug,
{
    fn expect(self, msg: &'static str) -> Pin<Box<dyn Future<Output = Self::Ok>>> {
        Box::pin(
            self.inspect_err(move |e| panic!("expect: {}; error={:?}", msg, e))
                .map(Result::unwrap),
        )
    }
}
