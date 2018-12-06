#![deny(warnings)]
extern crate http;
extern crate hyper;
#[macro_use]
extern crate futures;
extern crate futures_timer;
extern crate net2;
extern crate spmc;
extern crate pretty_env_logger;
extern crate tokio;
extern crate tokio_io;

use std::net::{TcpStream, Shutdown, SocketAddr};
use std::io::{self, Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc};
use std::net::{TcpListener as StdTcpListener};
use std::thread;
use std::time::Duration;

use futures::{Future, Stream};
use futures::future::{self, FutureResult, Either};
use futures::sync::oneshot;
use futures_timer::Delay;
use http::header::{HeaderName, HeaderValue};
use tokio::net::{TcpListener, TcpStream as TkTcpStream};
use tokio::runtime::current_thread::Runtime;
use tokio::reactor::Handle;
use tokio_io::{AsyncRead, AsyncWrite};

use hyper::{Body, Request, Response, StatusCode, Version};
use hyper::client::Client;
use hyper::server::conn::Http;
use hyper::server::Server;
use hyper::service::{service_fn, service_fn_ok, Service};


#[test]
fn get_should_ignore_body() {
    let server = serve();

    let mut req = connect(server.addr());
    // Connection: close = don't try to parse the body as a new request
    req.write_all(b"\
        GET / HTTP/1.1\r\n\
        Host: example.domain\r\n\
        Connection: close\r\n\
        \r\n\
        I shouldn't be read.\r\n\
    ").unwrap();
    req.read(&mut [0; 256]).unwrap();

    assert_eq!(server.body(), b"");
}

#[test]
fn get_with_body() {
    let server = serve();
    let mut req = connect(server.addr());
    req.write_all(b"\
        GET / HTTP/1.1\r\n\
        Host: example.domain\r\n\
        Content-Length: 19\r\n\
        \r\n\
        I'm a good request.\r\n\
    ").unwrap();
    req.read(&mut [0; 256]).unwrap();

    // note: doesn't include trailing \r\n, cause Content-Length wasn't 21
    assert_eq!(server.body(), b"I'm a good request.");
}

mod response_body_lengths {
    use super::*;

    struct TestCase {
        version: usize,
        headers: &'static [(&'static str, &'static str)],
        body: Bd,
        expects_chunked: bool,
        expects_con_len: bool,
    }

    enum Bd {
        Known(&'static str),
        Unknown(&'static str),
    }

    fn run_test(case: TestCase) {
        assert!(case.version == 0 || case.version == 1, "TestCase.version must 0 or 1");

        let server = serve();

        let mut reply = server.reply();
        for header in case.headers {
            reply = reply.header(header.0, header.1);
        }

        let body_str = match case.body {
            Bd::Known(b) => {
                reply.body(b);
                b
            },
            Bd::Unknown(b) => {
                let (mut tx, body) = hyper::Body::channel();
                tx.send_data(b.into()).expect("send_data");
                reply.body_stream(body);
                b
            },
        };

        let mut req = connect(server.addr());
        write!(req, "\
            GET / HTTP/1.{}\r\n\
            Host: example.domain\r\n\
            Connection: close\r\n\
            \r\n\
        ", case.version).expect("request write");
        let mut body = String::new();
        req.read_to_string(&mut body).unwrap();

        assert_eq!(
            case.expects_chunked,
            has_header(&body, "transfer-encoding:"),
            "expects_chunked"
        );

        assert_eq!(
            case.expects_chunked,
            has_header(&body, "chunked\r\n"),
            "expects_chunked"
        );

        assert_eq!(
            case.expects_con_len,
            has_header(&body, "content-length:"),
            "expects_con_len"
        );

        let n = body.find("\r\n\r\n").unwrap() + 4;

        if case.expects_chunked {
            let len = body.len();
            assert_eq!(&body[n + 1..n + 3], "\r\n", "expected body chunk size header");
            assert_eq!(&body[n + 3..len - 7], body_str, "expected body");
            assert_eq!(&body[len - 7..], "\r\n0\r\n\r\n", "expected body final chunk size header");
        } else {
            assert_eq!(&body[n..], body_str, "expected body");
        }
    }

    #[test]
    fn fixed_response_known() {
        run_test(TestCase {
            version: 1,
            headers: &[("content-length", "11")],
            body: Bd::Known("foo bar baz"),
            expects_chunked: false,
            expects_con_len: true,
        });
    }

    #[test]
    fn fixed_response_unknown() {
        run_test(TestCase {
            version: 1,
            headers: &[("content-length", "11")],
            body: Bd::Unknown("foo bar baz"),
            expects_chunked: false,
            expects_con_len: true,
        });
    }

    #[test]
    fn fixed_response_known_empty() {
        run_test(TestCase {
            version: 1,
            headers: &[("content-length", "0")],
            body: Bd::Known(""),
            expects_chunked: false,
            expects_con_len: true,
        });
    }

    #[test]
    fn chunked_response_known() {
        run_test(TestCase {
            version: 1,
            headers: &[("transfer-encoding", "chunked")],
            // even though we know the length, don't strip user's TE header
            body: Bd::Known("foo bar baz"),
            expects_chunked: true,
            expects_con_len: false,
        });
    }

    #[test]
    fn chunked_response_unknown() {
        run_test(TestCase {
            version: 1,
            headers: &[("transfer-encoding", "chunked")],
            body: Bd::Unknown("foo bar baz"),
            expects_chunked: true,
            expects_con_len: false,
        });
    }

    #[test]
    fn te_response_adds_chunked() {
        run_test(TestCase {
            version: 1,
            headers: &[("transfer-encoding", "gzip")],
            body: Bd::Unknown("foo bar baz"),
            expects_chunked: true,
            expects_con_len: false,
        });
    }

    #[test]
    #[ignore]
    // This used to be the case, but providing this functionality got in the
    // way of performance. It can probably be brought back later, and doing
    // so should be backwards-compatible...
    fn chunked_response_trumps_length() {
        run_test(TestCase {
            version: 1,
            headers: &[
                ("transfer-encoding", "chunked"),
                // both headers means content-length is stripped
                ("content-length", "11"),
            ],
            body: Bd::Known("foo bar baz"),
            expects_chunked: true,
            expects_con_len: false,
        });
    }

    #[test]
    fn auto_response_with_unknown_length() {
        run_test(TestCase {
            version: 1,
            // no headers means trying to guess from Payload
            headers: &[],
            body: Bd::Unknown("foo bar baz"),
            expects_chunked: true,
            expects_con_len: false,
        });
    }

    #[test]
    fn auto_response_with_known_length() {
        run_test(TestCase {
            version: 1,
            // no headers means trying to guess from Payload
            headers: &[],
            body: Bd::Known("foo bar baz"),
            expects_chunked: false,
            expects_con_len: true,
        });
    }

    #[test]
    fn auto_response_known_empty() {
        run_test(TestCase {
            version: 1,
            // no headers means trying to guess from Payload
            headers: &[],
            body: Bd::Known(""),
            expects_chunked: false,
            expects_con_len: true,
        });
    }

    #[test]
    fn http10_auto_response_with_unknown_length() {
        run_test(TestCase {
            version: 0,
            // no headers means trying to guess from Payload
            headers: &[],
            body: Bd::Unknown("foo bar baz"),
            expects_chunked: false,
            expects_con_len: false,
        });
    }


    #[test]
    fn http10_chunked_response() {
        run_test(TestCase {
            version: 0,
            // http/1.0 should strip this header
            headers: &[("transfer-encoding", "chunked")],
            // even when we don't know the length
            body: Bd::Unknown("foo bar baz"),
            expects_chunked: false,
            expects_con_len: false,
        });
    }

    #[test]
    fn http2_auto_response_with_known_length() {
        use hyper::body::Payload;

        let server = serve();
        let addr_str = format!("http://{}", server.addr());
        server.reply().body("Hello, World!");

        let mut rt = Runtime::new().expect("rt new");
        rt.block_on(hyper::rt::lazy(move || {
            let client = Client::builder()
                .http2_only(true)
                .build_http::<hyper::Body>();
            let uri = addr_str
                .parse::<hyper::Uri>()
                .expect("server addr should parse");

            client
                .get(uri)
                .and_then(|res| {
                    assert_eq!(res.headers().get("content-length").unwrap(), "13");
                    assert_eq!(res.body().content_length(), Some(13));
                    Ok(())
                })
                .map(|_| ())
                .map_err(|_e| ())
        })).unwrap();
    }

    #[test]
    fn http2_auto_response_with_conflicting_lengths() {
        use hyper::body::Payload;

        let server = serve();
        let addr_str = format!("http://{}", server.addr());
        server
            .reply()
            .header("content-length", "10")
            .body("Hello, World!");

        let mut rt = Runtime::new().expect("rt new");
        rt.block_on(hyper::rt::lazy(move || {
            let client = Client::builder()
                .http2_only(true)
                .build_http::<hyper::Body>();
            let uri = addr_str
                .parse::<hyper::Uri>()
                .expect("server addr should parse");

            client
                .get(uri)
                .and_then(|res| {
                    assert_eq!(res.headers().get("content-length").unwrap(), "10");
                    assert_eq!(res.body().content_length(), Some(10));
                    Ok(())
                })
                .map(|_| ())
                .map_err(|_e| ())
        })).unwrap();
    }
}

#[test]
fn get_chunked_response_with_ka() {
    let foo_bar = b"foo bar baz";
    let foo_bar_chunk = b"\r\nfoo bar baz\r\n0\r\n\r\n";
    let server = serve();
    server.reply()
        .header("transfer-encoding", "chunked")
        .body(foo_bar);
    let mut req = connect(server.addr());
    req.write_all(b"\
        GET / HTTP/1.1\r\n\
        Host: example.domain\r\n\
        Connection: keep-alive\r\n\
        \r\n\
    ").expect("writing 1");

    read_until(&mut req, |buf| {
        buf.ends_with(foo_bar_chunk)
    }).expect("reading 1");

    // try again!

    let quux = b"zar quux";
    server.reply()
        .header("content-length", quux.len().to_string())
        .body(quux);
    req.write_all(b"\
        GET /quux HTTP/1.1\r\n\
        Host: example.domain\r\n\
        Connection: close\r\n\
        \r\n\
    ").expect("writing 2");


    read_until(&mut req, |buf| {
        buf.ends_with(quux)
    }).expect("reading 2");
}

#[test]
fn post_with_chunked_body() {
    let server = serve();
    let mut req = connect(server.addr());
    req.write_all(b"\
        POST / HTTP/1.1\r\n\
        Host: example.domain\r\n\
        Transfer-Encoding: chunked\r\n\
        \r\n\
        1\r\n\
        q\r\n\
        2\r\n\
        we\r\n\
        2\r\n\
        rt\r\n\
        0\r\n\
        \r\n\
    ").unwrap();
    req.read(&mut [0; 256]).unwrap();

    assert_eq!(server.body(), b"qwert");
}

#[test]
fn post_with_incomplete_body() {
    let _ = pretty_env_logger::try_init();
    let server = serve();
    let mut req = connect(server.addr());
    req.write_all(b"\
        POST / HTTP/1.1\r\n\
        Host: example.domain\r\n\
        Content-Length: 10\r\n\
        \r\n\
        12345\
    ").expect("write");
    req.shutdown(Shutdown::Write).expect("shutdown write");

    server.body_err();

    req.read(&mut [0; 256]).expect("read");
}


#[test]
fn head_response_can_send_content_length() {
    let _ = pretty_env_logger::try_init();
    let server = serve();
    server.reply()
        .header("content-length", "1024");
    let mut req = connect(server.addr());
    req.write_all(b"\
        HEAD / HTTP/1.1\r\n\
        Host: example.domain\r\n\
        Connection: close\r\n\
        \r\n\
    ").unwrap();

    let mut response = String::new();
    req.read_to_string(&mut response).unwrap();

    assert!(response.contains("content-length: 1024\r\n"));

    let mut lines = response.lines();
    assert_eq!(lines.next(), Some("HTTP/1.1 200 OK"));

    let mut lines = lines.skip_while(|line| !line.is_empty());
    assert_eq!(lines.next(), Some(""));
    assert_eq!(lines.next(), None);
}

#[test]
fn head_response_doesnt_send_body() {
    let _ = pretty_env_logger::try_init();
    let foo_bar = b"foo bar baz";
    let server = serve();
    server.reply()
        .body(foo_bar);
    let mut req = connect(server.addr());
    req.write_all(b"\
        HEAD / HTTP/1.1\r\n\
        Host: example.domain\r\n\
        Connection: close\r\n\
        \r\n\
    ").unwrap();

    let mut response = String::new();
    req.read_to_string(&mut response).unwrap();

    assert!(response.contains("content-length: 11\r\n"));

    let mut lines = response.lines();
    assert_eq!(lines.next(), Some("HTTP/1.1 200 OK"));

    let mut lines = lines.skip_while(|line| !line.is_empty());
    assert_eq!(lines.next(), Some(""));
    assert_eq!(lines.next(), None);
}

#[test]
fn response_does_not_set_chunked_if_body_not_allowed() {
    let _ = pretty_env_logger::try_init();
    let server = serve();
    server.reply()
        .status(hyper::StatusCode::NOT_MODIFIED)
        .header("transfer-encoding", "chunked");
    let mut req = connect(server.addr());
    req.write_all(b"\
        GET / HTTP/1.1\r\n\
        Host: example.domain\r\n\
        Connection: close\r\n\
        \r\n\
    ").unwrap();

    let mut response = String::new();
    req.read_to_string(&mut response).unwrap();

    assert!(!response.contains("transfer-encoding"));

    let mut lines = response.lines();
    assert_eq!(lines.next(), Some("HTTP/1.1 304 Not Modified"));

    // no body or 0\r\n\r\n
    let mut lines = lines.skip_while(|line| !line.is_empty());
    assert_eq!(lines.next(), Some(""));
    assert_eq!(lines.next(), None);
}

#[test]
fn keep_alive() {
    let foo_bar = b"foo bar baz";
    let server = serve();
    server.reply()
        .header("content-length", foo_bar.len().to_string())
        .body(foo_bar);
    let mut req = connect(server.addr());
    req.write_all(b"\
        GET / HTTP/1.1\r\n\
        Host: example.domain\r\n\
        \r\n\
    ").expect("writing 1");

    read_until(&mut req, |buf| {
        buf.ends_with(foo_bar)
    }).expect("reading 1");

    // try again!

    let quux = b"zar quux";
    server.reply()
        .header("content-length", quux.len().to_string())
        .body(quux);
    req.write_all(b"\
        GET /quux HTTP/1.1\r\n\
        Host: example.domain\r\n\
        Connection: close\r\n\
        \r\n\
    ").expect("writing 2");

    read_until(&mut req, |buf| {
        buf.ends_with(quux)
    }).expect("reading 2");
}

#[test]
fn http_10_keep_alive() {
    let foo_bar = b"foo bar baz";
    let server = serve();
    // Response version 1.1 with no keep-alive header will downgrade to 1.0 when served
    server.reply()
        .header("content-length", foo_bar.len().to_string())
        .body(foo_bar);
    let mut req = connect(server.addr());
    req.write_all(b"\
        GET / HTTP/1.0\r\n\
        Host: example.domain\r\n\
        Connection: keep-alive\r\n\
        \r\n\
    ").expect("writing 1");


    // Connection: keep-alive header should be added when downgrading to a 1.0 response
    let res = read_until(&mut req, |buf| {
        buf.ends_with(foo_bar)
    }).expect("reading 1");

    let sres = s(&res);
    assert!(
        sres.contains("connection: keep-alive\r\n"),
        "HTTP/1.0 response should have sent keep-alive: {:?}",
        sres,
    );

    // try again!

    let quux = b"zar quux";
    server.reply()
        .header("content-length", quux.len().to_string())
        .body(quux);
    req.write_all(b"\
        GET /quux HTTP/1.0\r\n\
        Host: example.domain\r\n\
        \r\n\
    ").expect("writing 2");


    read_until(&mut req, |buf| {
        buf.ends_with(quux)
    }).expect("reading 2");
}

#[test]
fn http_10_close_on_no_ka() {
    let foo_bar = b"foo bar baz";
    let server = serve();

    // A server response with version 1.0 but no keep-alive header
    server
        .reply()
        .version(Version::HTTP_10)
        .header("content-length", foo_bar.len().to_string())
        .body(foo_bar);
    let mut req = connect(server.addr());

    // The client request with version 1.0 that may have the keep-alive header
    req.write_all(
        b"\
        GET / HTTP/1.0\r\n\
        Host: example.domain\r\n\
        Connection: keep-alive\r\n\
        \r\n\
    ",
    ).expect("writing 1");

    // server isn't keeping-alive, so the socket should be closed after
    // writing the response. thus, read_to_end should succeed.
    let mut buf = Vec::new();
    req.read_to_end(&mut buf).expect("reading 1");

    assert!(buf.ends_with(foo_bar));
    let sbuf = s(&buf);
    assert!(
        !sbuf.contains("connection: keep-alive\r\n"),
        "HTTP/1.0 response shouldn't have sent keep-alive: {:?}",
        sbuf,
    );
}

#[test]
fn disable_keep_alive() {
    let foo_bar = b"foo bar baz";
    let server = serve_opts()
        .keep_alive(false)
        .serve();
    server.reply()
        .header("content-length", foo_bar.len().to_string())
        .body(foo_bar);
    let mut req = connect(server.addr());
    req.write_all(b"\
        GET / HTTP/1.1\r\n\
        Host: example.domain\r\n\
        Connection: keep-alive\r\n\
        \r\n\
    ").expect("writing 1");


    // server isn't keeping-alive, so the socket should be closed after
    // writing the response. thus, read_to_end should succeed.
    let mut buf = Vec::new();
    req.read_to_end(&mut buf).expect("reading 1");
    assert!(buf.ends_with(foo_bar));
}

#[test]
fn header_connection_close() {
    let foo_bar = b"foo bar baz";
    let server = serve();
    server.reply()
        .header("content-length", foo_bar.len().to_string())
        .header("connection", "close")
        .body(foo_bar);
    let mut req = connect(server.addr());
    req.write_all(b"\
        GET / HTTP/1.1\r\n\
        Host: example.domain\r\n\
        Connection: keep-alive\r\n\
        \r\n\
    ").expect("writing 1");

    // server isn't keeping-alive, so the socket should be closed after
    // writing the response. thus, read_to_end should succeed.
    let mut buf = Vec::new();
    req.read_to_end(&mut buf).expect("reading 1");
    assert!(buf.ends_with(foo_bar));
    let sbuf = s(&buf);
    assert!(
        sbuf.contains("connection: close\r\n"),
        "response should have sent close: {:?}",
        sbuf,
    );
}

#[test]
fn expect_continue_sends_100() {
    let server = serve();
    let mut req = connect(server.addr());
    server.reply();

    req.write_all(b"\
        POST /foo HTTP/1.1\r\n\
        Host: example.domain\r\n\
        Expect: 100-continue\r\n\
        Content-Length: 5\r\n\
        Connection: Close\r\n\
        \r\n\
    ").expect("write 1");

    let msg = b"HTTP/1.1 100 Continue\r\n\r\n";
    let mut buf = vec![0; msg.len()];
    req.read_exact(&mut buf).expect("read 1");
    assert_eq!(buf, msg);

    let msg = b"hello";
    req.write_all(msg).expect("write 2");

    let mut body = String::new();
    req.read_to_string(&mut body).expect("read 2");

    let body = server.body();
    assert_eq!(body, msg);
}

#[test]
fn pipeline_disabled() {
    let server = serve();
    let mut req = connect(server.addr());
    server.reply()
        .header("content-length", "12")
        .body("Hello World!");
    server.reply()
        .header("content-length", "12")
        .body("Hello World!");

    req.write_all(b"\
        GET / HTTP/1.1\r\n\
        Host: example.domain\r\n\
        \r\n\
        GET / HTTP/1.1\r\n\
        Host: example.domain\r\n\
        \r\n\
    ").expect("write 1");

    let mut buf = vec![0; 4096];
    let n = req.read(&mut buf).expect("read 1");
    assert_ne!(n, 0);
    // Woah there. What?
    //
    // This test is wishy-washy because of race conditions in access of the
    // socket. The test is still useful, since it allows for the responses
    // to be received in 2 reads. But it might sometimes come in 1 read.
    //
    // TODO: add in a delay to the `ServeReply` interface, to allow this
    // delay to prevent the 2 writes from happening before this test thread
    // can read from the socket.
    match req.read(&mut buf) {
        Ok(n) => {
            // won't be 0, because we didn't say to close, and so socket
            // will be open until `server` drops
            assert_ne!(n, 0);
        }
        Err(_) => (),
    }
}

#[test]
fn pipeline_enabled() {
    let server = serve_opts()
        .pipeline(true)
        .serve();
    let mut req = connect(server.addr());
    server.reply()
        .header("content-length", "12")
        .body("Hello World\n");
    server.reply()
        .header("content-length", "12")
        .body("Hello World\n");

    req.write_all(b"\
        GET / HTTP/1.1\r\n\
        Host: example.domain\r\n\
        \r\n\
        GET / HTTP/1.1\r\n\
        Host: example.domain\r\n\
        Connection: close\r\n\
        \r\n\
    ").expect("write 1");

    let mut buf = vec![0; 4096];
    let n = req.read(&mut buf).expect("read 1");
    assert_ne!(n, 0);

    {
        let mut lines = buf.split(|&b| b == b'\n');
        assert_eq!(s(lines.next().unwrap()), "HTTP/1.1 200 OK\r");
        assert_eq!(s(lines.next().unwrap()), "content-length: 12\r");
        lines.next().unwrap(); // Date
        assert_eq!(s(lines.next().unwrap()), "\r");
        assert_eq!(s(lines.next().unwrap()), "Hello World");

        assert_eq!(s(lines.next().unwrap()), "HTTP/1.1 200 OK\r");
        assert_eq!(s(lines.next().unwrap()), "content-length: 12\r");
        lines.next().unwrap(); // Date
        assert_eq!(s(lines.next().unwrap()), "\r");
        assert_eq!(s(lines.next().unwrap()), "Hello World");
    }


    // with pipeline enabled, both responses should have been in the first read
    // so a second read should be EOF
    let n = req.read(&mut buf).expect("read 2");
    assert_eq!(n, 0);
}

#[test]
fn http_10_request_receives_http_10_response() {
    let server = serve();

    let mut req = connect(server.addr());
    req.write_all(b"\
        GET / HTTP/1.0\r\n\
        \r\n\
    ").unwrap();

    let expected = "HTTP/1.0 200 OK\r\ncontent-length: 0\r\n";
    let mut buf = [0; 256];
    let n = req.read(&mut buf).unwrap();
    assert!(n >= expected.len(), "read: {:?} >= {:?}", n, expected.len());
    assert_eq!(s(&buf[..expected.len()]), expected);
}

#[test]
fn disable_keep_alive_mid_request() {
    let mut rt = Runtime::new().unwrap();
    let listener = tcp_bind(&"127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = listener.local_addr().unwrap();

    let (tx1, rx1) = oneshot::channel();
    let (tx2, rx2) = oneshot::channel();

    let child = thread::spawn(move || {
        let mut req = connect(&addr);
        req.write_all(b"GET / HTTP/1.1\r\n").unwrap();
        tx1.send(()).unwrap();
        rx2.wait().unwrap();
        req.write_all(b"Host: localhost\r\n\r\n").unwrap();
        let mut buf = vec![];
        req.read_to_end(&mut buf).unwrap();
    });

    let fut = listener.incoming()
        .into_future()
        .map_err(|_| unreachable!())
        .and_then(|(item, _incoming)| {
            let socket = item.unwrap();
            Http::new().serve_connection(socket, HelloWorld)
                .select2(rx1)
                .then(|r| {
                    match r {
                        Ok(Either::A(_)) => panic!("expected rx first"),
                        Ok(Either::B(((), mut conn))) => {
                            conn.graceful_shutdown();
                            tx2.send(()).unwrap();
                            conn
                        }
                        Err(Either::A((e, _))) => panic!("unexpected error {}", e),
                        Err(Either::B((e, _))) => panic!("unexpected error {}", e),
                    }
                })
        });

    rt.block_on(fut).unwrap();
    child.join().unwrap();
}

#[test]
fn disable_keep_alive_post_request() {
    let _ = pretty_env_logger::try_init();
    let mut rt = Runtime::new().unwrap();
    let listener = tcp_bind(&"127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = listener.local_addr().unwrap();

    let (tx1, rx1) = oneshot::channel();

    let child = thread::spawn(move || {
        let mut req = connect(&addr);
        req.write_all(b"\
            GET / HTTP/1.1\r\n\
            Host: localhost\r\n\
            \r\n\
        ").unwrap();

        read_until(&mut req, |buf| {
            buf.ends_with(HELLO.as_bytes())
        }).expect("reading 1");

        // Connection should get closed *after* tx is sent on
        tx1.send(()).unwrap();

        let nread = req.read(&mut [0u8; 1024]).expect("keep-alive reading");
        assert_eq!(nread, 0);
    });

    let dropped = Dropped::new();
    let dropped2 = dropped.clone();
    let fut = listener.incoming()
        .into_future()
        .map_err(|_| unreachable!())
        .and_then(|(item, _incoming)| {
            let socket = item.expect("accepted socket");
            let transport = DebugStream {
                stream: socket,
                _debug: dropped2,
            };
            Http::new().serve_connection(transport, HelloWorld)
                .select2(rx1)
                .then(|r| {
                    match r {
                        Ok(Either::A(_)) => panic!("expected rx first"),
                        Ok(Either::B(((), mut conn))) => {
                            conn.graceful_shutdown();
                            conn
                        }
                        Err(Either::A((e, _))) => panic!("unexpected error {}", e),
                        Err(Either::B((e, _))) => panic!("unexpected error {}", e),
                    }
                })
        });

    assert!(!dropped.load());
    rt.block_on(fut).unwrap();
    // we must poll the Core one more time in order for Windows to drop
    // the read-blocked socket.
    //
    // See https://github.com/carllerche/mio/issues/776
    let timeout = Delay::new(Duration::from_millis(10));
    rt.block_on(timeout).unwrap();
    assert!(dropped.load());
    child.join().unwrap();
}

#[test]
fn empty_parse_eof_does_not_return_error() {
    let mut rt = Runtime::new().unwrap();
    let listener = tcp_bind(&"127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = listener.local_addr().unwrap();

    thread::spawn(move || {
        let _tcp = connect(&addr);
    });

    let fut = listener.incoming()
        .into_future()
        .map_err(|_| unreachable!())
        .and_then(|(item, _incoming)| {
            let socket = item.unwrap();
            Http::new().serve_connection(socket, HelloWorld)
        });

    rt.block_on(fut).expect("empty parse eof is ok");
}

#[test]
fn nonempty_parse_eof_returns_error() {
    let mut rt = Runtime::new().unwrap();
    let listener = tcp_bind(&"127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = listener.local_addr().unwrap();

    thread::spawn(move || {
        let mut tcp = connect(&addr);
        tcp.write_all(b"GET / HTTP/1.1").unwrap();
    });

    let fut = listener.incoming()
        .into_future()
        .map_err(|_| unreachable!())
        .and_then(|(item, _incoming)| {
            let socket = item.unwrap();
            Http::new().serve_connection(socket, HelloWorld)
        });

    rt.block_on(fut).expect_err("partial parse eof is error");
}

#[test]
fn socket_half_closed() {
    let _ = pretty_env_logger::try_init();
    let mut rt = Runtime::new().unwrap();
    let listener = tcp_bind(&"127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = listener.local_addr().unwrap();

    thread::spawn(move || {
        let mut tcp = connect(&addr);
        tcp.write_all(b"GET / HTTP/1.1\r\n\r\n").unwrap();
        tcp.shutdown(::std::net::Shutdown::Write).expect("SHDN_WR");

        let mut buf = [0; 256];
        tcp.read(&mut buf).unwrap();
        let expected = "HTTP/1.1 200 OK\r\n";
        assert_eq!(s(&buf[..expected.len()]), expected);
    });

    let fut = listener.incoming()
        .into_future()
        .map_err(|_| unreachable!())
        .and_then(|(item, _incoming)| {
            let socket = item.unwrap();
            Http::new()
                .serve_connection(socket, service_fn(|_| {
                    Delay::new(Duration::from_millis(500))
                        .map(|_| {
                            Response::new(Body::empty())
                        })
                }))
        });

    rt.block_on(fut).unwrap();
}

#[test]
fn disconnect_after_reading_request_before_responding() {
    let _ = pretty_env_logger::try_init();
    let mut rt = Runtime::new().unwrap();
    let listener = tcp_bind(&"127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = listener.local_addr().unwrap();

    thread::spawn(move || {
        let mut tcp = connect(&addr);
        tcp.write_all(b"GET / HTTP/1.1\r\n\r\n").unwrap();
    });

    let fut = listener.incoming()
        .into_future()
        .map_err(|_| unreachable!())
        .and_then(|(item, _incoming)| {
            let socket = item.unwrap();
            Http::new()
                .http1_half_close(false)
                .serve_connection(socket, service_fn(|_| {
                    Delay::new(Duration::from_secs(2))
                        .map(|_| -> Response<Body> {
                            panic!("response future should have been dropped");
                        })
                }))
        });

    rt.block_on(fut).expect_err("socket disconnected");
}

#[test]
fn returning_1xx_response_is_error() {
    let mut rt = Runtime::new().unwrap();
    let listener = tcp_bind(&"127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = listener.local_addr().unwrap();

    thread::spawn(move || {
        let mut tcp = connect(&addr);
        tcp.write_all(b"GET / HTTP/1.1\r\n\r\n").unwrap();
        let mut buf = [0; 256];
        tcp.read(&mut buf).unwrap();

        let expected = "HTTP/1.1 500 ";
        assert_eq!(s(&buf[..expected.len()]), expected);
    });

    let fut = listener.incoming()
        .into_future()
        .map_err(|_| unreachable!())
        .and_then(|(item, _incoming)| {
            let socket = item.unwrap();
            Http::new()
                .serve_connection(socket, service_fn(|_| {
                    Ok::<_, hyper::Error>(Response::builder()
                        .status(StatusCode::CONTINUE)
                        .body(Body::empty())
                        .unwrap())
                }))
        });

    rt.block_on(fut).expect_err("1xx status code should error");
}

#[test]
fn header_name_too_long() {
    let server = serve();

    let mut req = connect(server.addr());
    let mut write = Vec::with_capacity(1024 * 66);
    write.extend_from_slice(b"GET / HTTP/1.1\r\n");
    for _ in 0..(1024 * 65) {
        write.push(b'x');
    }
    write.extend_from_slice(b": foo\r\n\r\n");
    req.write_all(&write).unwrap();

    let mut buf = [0; 1024];
    let n = req.read(&mut buf).unwrap();
    assert!(s(&buf[..n]).starts_with("HTTP/1.1 431 Request Header Fields Too Large\r\n"));
}

#[test]
fn upgrades() {
    use tokio_io::io::{read_to_end, write_all};
    let _ = pretty_env_logger::try_init();
    let mut rt = Runtime::new().unwrap();
    let listener = tcp_bind(&"127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = oneshot::channel();

    thread::spawn(move || {
        let mut tcp = connect(&addr);
        tcp.write_all(b"\
            GET / HTTP/1.1\r\n\
            Upgrade: foobar\r\n\
            Connection: upgrade\r\n\
            \r\n\
            eagerly optimistic\
        ").expect("write 1");
        let mut buf = [0; 256];
        tcp.read(&mut buf).expect("read 1");

        let expected = "HTTP/1.1 101 Switching Protocols\r\n";
        assert_eq!(s(&buf[..expected.len()]), expected);
        let _ = tx.send(());

        let n = tcp.read(&mut buf).expect("read 2");
        assert_eq!(s(&buf[..n]), "foo=bar");
        tcp.write_all(b"bar=foo").expect("write 2");
    });

    let fut = listener.incoming()
        .into_future()
        .map_err(|_| -> hyper::Error { unreachable!() })
        .and_then(|(item, _incoming)| {
            let socket = item.unwrap();
            let conn = Http::new()
                .serve_connection(socket, service_fn(|_| {
                    let res = Response::builder()
                        .status(101)
                        .header("upgrade", "foobar")
                        .body(hyper::Body::empty())
                        .unwrap();
                    Ok::<_, hyper::Error>(res)
                }));

            let mut conn_opt = Some(conn);
            future::poll_fn(move || {
                try_ready!(conn_opt.as_mut().unwrap().poll_without_shutdown());
                // conn is done with HTTP now
                Ok(conn_opt.take().unwrap().into())
            })
        });

    let conn = rt.block_on(fut).unwrap();

    // wait so that we don't write until other side saw 101 response
    rt.block_on(rx).unwrap();

    let parts = conn.into_parts();
    let io = parts.io;
    assert_eq!(parts.read_buf, "eagerly optimistic");

    let io = rt.block_on(write_all(io, b"foo=bar")).unwrap().0;
    let vec = rt.block_on(read_to_end(io, vec![])).unwrap().1;
    assert_eq!(vec, b"bar=foo");
}

#[test]
fn http_connect() {
    use tokio_io::io::{read_to_end, write_all};
    let _ = pretty_env_logger::try_init();
    let mut rt = Runtime::new().unwrap();
    let listener = tcp_bind(&"127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = oneshot::channel();

    thread::spawn(move || {
        let mut tcp = connect(&addr);
        tcp.write_all(b"\
            CONNECT localhost:80 HTTP/1.1\r\n\
            \r\n\
            eagerly optimistic\
        ").expect("write 1");
        let mut buf = [0; 256];
        tcp.read(&mut buf).expect("read 1");

        let expected = "HTTP/1.1 200 OK\r\n";
        assert_eq!(s(&buf[..expected.len()]), expected);
        let _ = tx.send(());

        let n = tcp.read(&mut buf).expect("read 2");
        assert_eq!(s(&buf[..n]), "foo=bar");
        tcp.write_all(b"bar=foo").expect("write 2");
    });

    let fut = listener.incoming()
        .into_future()
        .map_err(|_| -> hyper::Error { unreachable!() })
        .and_then(|(item, _incoming)| {
            let socket = item.unwrap();
            let conn = Http::new()
                .serve_connection(socket, service_fn(|_| {
                    let res = Response::builder()
                        .status(200)
                        .body(hyper::Body::empty())
                        .unwrap();
                    Ok::<_, hyper::Error>(res)
                }));

            let mut conn_opt = Some(conn);
            future::poll_fn(move || {
                try_ready!(conn_opt.as_mut().unwrap().poll_without_shutdown());
                // conn is done with HTTP now
                Ok(conn_opt.take().unwrap().into())
            })
        });

    let conn = rt.block_on(fut).unwrap();

    // wait so that we don't write until other side saw 101 response
    rt.block_on(rx).unwrap();

    let parts = conn.into_parts();
    let io = parts.io;
    assert_eq!(parts.read_buf, "eagerly optimistic");

    let io = rt.block_on(write_all(io, b"foo=bar")).unwrap().0;
    let vec = rt.block_on(read_to_end(io, vec![])).unwrap().1;
    assert_eq!(vec, b"bar=foo");
}

#[test]
fn upgrades_new() {
    use tokio_io::io::{read_to_end, write_all};
    let _ = pretty_env_logger::try_init();
    let mut rt = Runtime::new().unwrap();
    let listener = tcp_bind(&"127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = listener.local_addr().unwrap();
    let (read_101_tx, read_101_rx) = oneshot::channel();

    thread::spawn(move || {
        let mut tcp = connect(&addr);
        tcp.write_all(b"\
            GET / HTTP/1.1\r\n\
            Upgrade: foobar\r\n\
            Connection: upgrade\r\n\
            \r\n\
            eagerly optimistic\
        ").expect("write 1");
        let mut buf = [0; 256];
        tcp.read(&mut buf).expect("read 1");

        let expected = "HTTP/1.1 101 Switching Protocols\r\n";
        assert_eq!(s(&buf[..expected.len()]), expected);
        let _ = read_101_tx.send(());

        let n = tcp.read(&mut buf).expect("read 2");
        assert_eq!(s(&buf[..n]), "foo=bar");
        tcp.write_all(b"bar=foo").expect("write 2");
    });

    let (upgrades_tx, upgrades_rx) = mpsc::channel();
    let svc = service_fn_ok(move |req: Request<Body>| {
        let on_upgrade = req
            .into_body()
            .on_upgrade();
        let _ = upgrades_tx.send(on_upgrade);
        Response::builder()
            .status(101)
            .header("upgrade", "foobar")
            .body(hyper::Body::empty())
            .unwrap()
    });

    let fut = listener.incoming()
        .into_future()
        .map_err(|_| -> hyper::Error { unreachable!() })
        .and_then(move |(item, _incoming)| {
            let socket = item.unwrap();
            Http::new()
                .serve_connection(socket, svc)
                .with_upgrades()
        });

    rt.block_on(fut).unwrap();
    let on_upgrade = upgrades_rx.recv().unwrap();

    // wait so that we don't write until other side saw 101 response
    rt.block_on(read_101_rx).unwrap();

    let upgraded = rt.block_on(on_upgrade).unwrap();
    let parts = upgraded.downcast::<TkTcpStream>().unwrap();
    let io = parts.io;
    assert_eq!(parts.read_buf, "eagerly optimistic");

    let io = rt.block_on(write_all(io, b"foo=bar")).unwrap().0;
    let vec = rt.block_on(read_to_end(io, vec![])).unwrap().1;
    assert_eq!(s(&vec), "bar=foo");
}

#[test]
fn http_connect_new() {
    use tokio_io::io::{read_to_end, write_all};
    let _ = pretty_env_logger::try_init();
    let mut rt = Runtime::new().unwrap();
    let listener = tcp_bind(&"127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = listener.local_addr().unwrap();
    let (read_200_tx, read_200_rx) = oneshot::channel();

    thread::spawn(move || {
        let mut tcp = connect(&addr);
        tcp.write_all(b"\
            CONNECT localhost HTTP/1.1\r\n\
            \r\n\
            eagerly optimistic\
        ").expect("write 1");
        let mut buf = [0; 256];
        tcp.read(&mut buf).expect("read 1");

        let expected = "HTTP/1.1 200 OK\r\n";
        assert_eq!(s(&buf[..expected.len()]), expected);
        let _ = read_200_tx.send(());

        let n = tcp.read(&mut buf).expect("read 2");
        assert_eq!(s(&buf[..n]), "foo=bar");
        tcp.write_all(b"bar=foo").expect("write 2");
    });

    let (upgrades_tx, upgrades_rx) = mpsc::channel();
    let svc = service_fn_ok(move |req: Request<Body>| {
        let on_upgrade = req
            .into_body()
            .on_upgrade();
        let _ = upgrades_tx.send(on_upgrade);
        Response::builder()
            .status(200)
            .body(hyper::Body::empty())
            .unwrap()
    });

    let fut = listener.incoming()
        .into_future()
        .map_err(|_| -> hyper::Error { unreachable!() })
        .and_then(move |(item, _incoming)| {
            let socket = item.unwrap();
            Http::new()
                .serve_connection(socket, svc)
                .with_upgrades()
        });

    rt.block_on(fut).unwrap();
    let on_upgrade = upgrades_rx.recv().unwrap();

    // wait so that we don't write until other side saw 200
    rt.block_on(read_200_rx).unwrap();

    let upgraded = rt.block_on(on_upgrade).unwrap();
    let parts = upgraded.downcast::<TkTcpStream>().unwrap();
    let io = parts.io;
    assert_eq!(parts.read_buf, "eagerly optimistic");

    let io = rt.block_on(write_all(io, b"foo=bar")).unwrap().0;
    let vec = rt.block_on(read_to_end(io, vec![])).unwrap().1;
    assert_eq!(s(&vec), "bar=foo");
}


#[test]
fn parse_errors_send_4xx_response() {
    let mut rt = Runtime::new().unwrap();
    let listener = tcp_bind(&"127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = listener.local_addr().unwrap();

    thread::spawn(move || {
        let mut tcp = connect(&addr);
        tcp.write_all(b"GE T / HTTP/1.1\r\n\r\n").unwrap();
        let mut buf = [0; 256];
        tcp.read(&mut buf).unwrap();

        let expected = "HTTP/1.1 400 ";
        assert_eq!(s(&buf[..expected.len()]), expected);
    });

    let fut = listener.incoming()
        .into_future()
        .map_err(|_| unreachable!())
        .and_then(|(item, _incoming)| {
            let socket = item.unwrap();
            Http::new()
                .serve_connection(socket, HelloWorld)
        });

    rt.block_on(fut).expect_err("HTTP parse error");
}

#[test]
fn illegal_request_length_returns_400_response() {
    let mut rt = Runtime::new().unwrap();
    let listener = tcp_bind(&"127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = listener.local_addr().unwrap();

    thread::spawn(move || {
        let mut tcp = connect(&addr);
        tcp.write_all(b"POST / HTTP/1.1\r\nContent-Length: foo\r\n\r\n").unwrap();
        let mut buf = [0; 256];
        tcp.read(&mut buf).unwrap();

        let expected = "HTTP/1.1 400 ";
        assert_eq!(s(&buf[..expected.len()]), expected);
    });

    let fut = listener.incoming()
        .into_future()
        .map_err(|_| unreachable!())
        .and_then(|(item, _incoming)| {
            let socket = item.unwrap();
            Http::new()
                .serve_connection(socket, HelloWorld)
        });

    rt.block_on(fut).expect_err("illegal Content-Length should error");
}

#[test]
#[should_panic]
fn max_buf_size_panic_too_small() {
    const MAX: usize = 8191;
    Http::new().max_buf_size(MAX);
}
#[test]
fn max_buf_size_no_panic() {
    const MAX: usize = 8193;
    Http::new().max_buf_size(MAX);
}

#[test]
fn max_buf_size() {
    let _ = pretty_env_logger::try_init();
    let mut rt = Runtime::new().unwrap();
    let listener = tcp_bind(&"127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = listener.local_addr().unwrap();

    const MAX: usize = 16_000;

    thread::spawn(move || {
        let mut tcp = connect(&addr);
        tcp.write_all(b"POST /").expect("write 1");
        tcp.write_all(&vec![b'a'; MAX]).expect("write 2");
        let mut buf = [0; 256];
        tcp.read(&mut buf).expect("read 1");

        let expected = "HTTP/1.1 431 ";
        assert_eq!(s(&buf[..expected.len()]), expected);
    });

    let fut = listener.incoming()
        .into_future()
        .map_err(|_| unreachable!())
        .and_then(|(item, _incoming)| {
            let socket = item.unwrap();
            Http::new()
                .max_buf_size(MAX)
                .serve_connection(socket, HelloWorld)
        });

    rt.block_on(fut).expect_err("should TooLarge error");
}

#[test]
fn streaming_body() {
    let _ = pretty_env_logger::try_init();

    // disable keep-alive so we can use read_to_end
    let server = serve_opts()
        .keep_alive(false)
        .serve();

    static S: &'static [&'static [u8]] = &[&[b'x'; 1_000] as &[u8]; 1_00] as _;
    let b = ::futures::stream::iter_ok::<_, String>(S.into_iter())
        .map(|&s| s);
    let b = hyper::Body::wrap_stream(b);
    server
        .reply()
        .body_stream(b);

    let mut tcp = connect(server.addr());
    tcp.write_all(b"GET / HTTP/1.1\r\n\r\n").unwrap();
    let mut buf = Vec::new();
    tcp.read_to_end(&mut buf).expect("read 1");

    assert!(buf.starts_with(b"HTTP/1.1 200 OK\r\n"), "response is 200 OK");
    assert_eq!(buf.len(), 100_789, "full streamed body read");
}

#[test]
fn http1_response_with_http2_version() {
    let server = serve();
    let addr_str = format!("http://{}", server.addr());

    let mut rt = Runtime::new().expect("runtime new");

    server
        .reply()
        .version(hyper::Version::HTTP_2);

    rt.block_on(hyper::rt::lazy(move || {
        let client = Client::new();
        let uri = addr_str.parse().expect("server addr should parse");

        client.get(uri)
    })).unwrap();
}

#[test]
fn try_h2() {
    let server = serve();
    let addr_str = format!("http://{}", server.addr());

    let mut rt = Runtime::new().expect("runtime new");

    rt.block_on(hyper::rt::lazy(move || {
        let client = Client::builder()
            .http2_only(true)
            .build_http::<hyper::Body>();
        let uri = addr_str.parse().expect("server addr should parse");

        client.get(uri)
            .and_then(|_res| { Ok(()) })
            .map(|_| { () })
            .map_err(|_e| { () })
    })).unwrap();

    assert_eq!(server.body(), b"");
}

#[test]
fn http1_only() {
    let server = serve_opts()
        .http1_only()
        .serve();
    let addr_str = format!("http://{}", server.addr());

    let mut rt = Runtime::new().expect("runtime new");

    rt.block_on(hyper::rt::lazy(move || {
        let client = Client::builder()
            .http2_only(true)
            .build_http::<hyper::Body>();
        let uri = addr_str.parse().expect("server addr should parse");

        client.get(uri)
    })).unwrap_err();
}

// -------------------------------------------------
// the Server that is used to run all the tests with
// -------------------------------------------------

struct Serve {
    addr: SocketAddr,
    msg_rx: mpsc::Receiver<Msg>,
    reply_tx: spmc::Sender<Reply>,
    shutdown_signal: Option<oneshot::Sender<()>>,
    thread: Option<thread::JoinHandle<()>>,
}

impl Serve {
    fn addr(&self) -> &SocketAddr {
        &self.addr
    }

    fn body(&self) -> Vec<u8> {
        self.try_body().expect("body")
    }

    fn body_err(&self) -> hyper::Error {
        self.try_body().expect_err("body_err")
    }

    fn try_body(&self) -> Result<Vec<u8>, hyper::Error> {
        let mut buf = vec![];
        loop {
            match self.msg_rx.recv() {
                Ok(Msg::Chunk(msg)) => {
                    buf.extend(&msg);
                },
                Ok(Msg::Error(e)) => return Err(e),
                Ok(Msg::End) => break,
                Err(e) => panic!("expected body, found: {:?}", e),
            }
        }
        Ok(buf)
    }

    fn reply(&self) -> ReplyBuilder {
        ReplyBuilder {
            tx: &self.reply_tx
        }
    }
}

struct ReplyBuilder<'a> {
    tx: &'a spmc::Sender<Reply>,
}

impl<'a> ReplyBuilder<'a> {
    fn status(self, status: hyper::StatusCode) -> Self {
        self.tx.send(Reply::Status(status)).unwrap();
        self
    }

    fn version(self, version: hyper::Version) -> Self {
        self.tx.send(Reply::Version(version)).unwrap();
        self
    }

    fn header<V: AsRef<str>>(self, name: &str, value: V) -> Self {
        let name = HeaderName::from_bytes(name.as_bytes()).expect("header name");
        let value = HeaderValue::from_str(value.as_ref()).expect("header value");
        self.tx.send(Reply::Header(name, value)).unwrap();
        self
    }

    fn body<T: AsRef<[u8]>>(self, body: T) {
        self.tx.send(Reply::Body(body.as_ref().to_vec().into())).unwrap();
    }

    fn body_stream(self, body: Body)
    {
        self.tx.send(Reply::Body(body)).unwrap();
    }
}

impl<'a> Drop for ReplyBuilder<'a> {
    fn drop(&mut self) {
        let _ = self.tx.send(Reply::End);
    }
}

impl Drop for Serve {
    fn drop(&mut self) {
        drop(self.shutdown_signal.take());
        self.thread.take().unwrap().join().unwrap();
    }
}

#[derive(Clone)]
struct TestService {
    tx: mpsc::Sender<Msg>,
    reply: spmc::Receiver<Reply>,
}

#[derive(Debug)]
enum Reply {
    Status(hyper::StatusCode),
    Version(hyper::Version),
    Header(HeaderName, HeaderValue),
    Body(hyper::Body),
    End,
}

#[derive(Debug)]
enum Msg {
    Chunk(Vec<u8>),
    Error(hyper::Error),
    End,
}

impl TestService {
    fn call(&self, req: Request<Body>) -> Box<Future<Item=Response<Body>, Error=hyper::Error> + Send> {
        let tx1 = self.tx.clone();
        let tx2 = self.tx.clone();
        let replies = self.reply.clone();
        Box::new(req.into_body().for_each(move |chunk| {
            tx1.send(Msg::Chunk(chunk.to_vec())).unwrap();
            Ok(())
        }).then(move |result| {
            let msg = match result {
                Ok(()) => Msg::End,
                Err(e) => Msg::Error(e),
            };
            tx2.send(msg).unwrap();
            Ok(())
        }).map(move |_| {
            TestService::build_reply(replies)
        }))
    }

    fn build_reply(replies: spmc::Receiver<Reply>) -> Response<Body> {
        let mut res = Response::new(Body::empty());
        while let Ok(reply) = replies.try_recv() {
            match reply {
                Reply::Status(s) => {
                    *res.status_mut() = s;
                },
                Reply::Version(v) => {
                    *res.version_mut() = v;
                },
                Reply::Header(name, value) => {
                    res.headers_mut().insert(name, value);
                },
                Reply::Body(body) => {
                    *res.body_mut() = body;
                },
                Reply::End => break,
            }
        }
        res
    }
}

const HELLO: &'static str = "hello";

struct HelloWorld;

impl Service for HelloWorld {
    type ReqBody = Body;
    type ResBody = Body;
    type Error = hyper::Error;
    type Future = FutureResult<Response<Body>, Self::Error>;

    fn call(&mut self, _req: Request<Body>) -> Self::Future {
        let response = Response::new(HELLO.into());
        future::ok(response)
    }
}


fn connect(addr: &SocketAddr) -> TcpStream {
    let req = TcpStream::connect(addr).unwrap();
    req.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
    req.set_write_timeout(Some(Duration::from_secs(1))).unwrap();
    req
}

fn serve() -> Serve {
    serve_opts().serve()
}

fn serve_opts() -> ServeOptions {
    ServeOptions::default()
}

#[derive(Clone, Copy)]
struct ServeOptions {
    keep_alive: bool,
    http1_only: bool,
    pipeline: bool,
}

impl Default for ServeOptions {
    fn default() -> Self {
        ServeOptions {
            keep_alive: true,
            http1_only: false,
            pipeline: false,
        }
    }
}

impl ServeOptions {
    fn http1_only(mut self) -> Self {
        self.http1_only = true;
        self
    }

    fn keep_alive(mut self, enabled: bool) -> Self {
        self.keep_alive = enabled;
        self
    }

    fn pipeline(mut self, enabled: bool) -> Self {
        self.pipeline = enabled;
        self
    }

    fn serve(self) -> Serve {
        let _ = pretty_env_logger::try_init();
        let options = self;

        let (addr_tx, addr_rx) = mpsc::channel();
        let (msg_tx, msg_rx) = mpsc::channel();
        let (reply_tx, reply_rx) = spmc::channel();
        let (shutdown_tx, shutdown_rx) = oneshot::channel();

        let addr = ([127, 0, 0, 1], 0).into();

        let thread_name = format!(
            "test-server-{}",
            thread::current()
                .name()
                .unwrap_or("<unknown test case name>")
        );
        let thread = thread::Builder::new().name(thread_name).spawn(move || {
            let server = Server::bind(&addr)
                .http1_only(options.http1_only)
                .http1_keepalive(options.keep_alive)
                .http1_pipeline_flush(options.pipeline)
                .serve(move || {
                    let ts = TestService {
                        tx: msg_tx.clone(),
                        reply: reply_rx.clone(),
                    };
                    service_fn(move |req| ts.call(req))
                });

            addr_tx.send(
                server.local_addr()
            ).expect("server addr tx");

            let fut = server
                .with_graceful_shutdown(shutdown_rx);

            let mut rt = Runtime::new().expect("rt new");
            rt.block_on(fut).unwrap();
        }).expect("thread spawn");

        let addr = addr_rx.recv().expect("server addr rx");

        Serve {
            msg_rx: msg_rx,
            reply_tx: reply_tx,
            addr: addr,
            shutdown_signal: Some(shutdown_tx),
            thread: Some(thread),
        }
    }
}

fn s(buf: &[u8]) -> &str {
    ::std::str::from_utf8(buf).unwrap()
}

fn has_header(msg: &str, name: &str) -> bool {
    let n = msg.find("\r\n\r\n").unwrap_or(msg.len());

    msg[..n].contains(name)
}

fn tcp_bind(addr: &SocketAddr) -> ::tokio::io::Result<TcpListener> {
    let std_listener = StdTcpListener::bind(addr).unwrap();
    TcpListener::from_std(std_listener, &Handle::default())
}

fn read_until<R, F>(io: &mut R, func: F) -> io::Result<Vec<u8>>
where
    R: Read,
    F: Fn(&[u8]) -> bool,
{
    let mut buf = vec![0; 8192];
    let mut pos = 0;
    loop {
        let n = io.read(&mut buf[pos..])?;
        pos += n;
        if func(&buf[..pos]) {
            break;
        }

        if pos == buf.len() {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "read_until buffer filled"
            ));
        }
    }
    buf.truncate(pos);
    Ok(buf)
}

struct DebugStream<T, D> {
    stream: T,
    _debug: D,
}

impl<T: Read, D> Read for DebugStream<T, D> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.stream.read(buf)
    }
}

impl<T: Write, D> Write for DebugStream<T, D> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.stream.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.stream.flush()
    }
}


impl<T: AsyncWrite, D> AsyncWrite for DebugStream<T, D> {
    fn shutdown(&mut self) -> futures::Poll<(), io::Error> {
        self.stream.shutdown()
    }
}


impl<T: AsyncRead, D> AsyncRead for DebugStream<T, D> {}

#[derive(Clone)]
struct Dropped(Arc<AtomicBool>);

impl Dropped {
    pub fn new() -> Dropped {
        Dropped(Arc::new(AtomicBool::new(false)))
    }

    pub fn load(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

impl Drop for Dropped {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

