#![deny(warnings)]
#![deny(rust_2018_idioms)]

use std::future::Future;
use std::io::{self, Read, Write};
use std::net::TcpListener as StdTcpListener;
use std::net::{Shutdown, SocketAddr, TcpStream};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::thread;
use std::time::Duration;

use futures_channel::oneshot;
use futures_util::future::{self, Either, FutureExt, TryFutureExt};
#[cfg(feature = "stream")]
use futures_util::stream::StreamExt as _;
use http::header::{HeaderName, HeaderValue};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::{TcpListener, TcpStream as TkTcpStream};
use tokio::runtime::Runtime;

use hyper::body::HttpBody as _;
use hyper::client::Client;
use hyper::server::conn::Http;
use hyper::server::Server;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, StatusCode, Version};

#[test]
fn get_should_ignore_body() {
    let server = serve();

    let mut req = connect(server.addr());
    // Connection: close = don't try to parse the body as a new request
    req.write_all(
        b"\
        GET / HTTP/1.1\r\n\
        Host: example.domain\r\n\
        Connection: close\r\n\
        \r\n\
        I shouldn't be read.\r\n\
    ",
    )
    .unwrap();
    req.read(&mut [0; 256]).unwrap();

    assert_eq!(server.body(), b"");
}

#[test]
fn get_with_body() {
    let server = serve();
    let mut req = connect(server.addr());
    req.write_all(
        b"\
        GET / HTTP/1.1\r\n\
        Host: example.domain\r\n\
        Content-Length: 19\r\n\
        \r\n\
        I'm a good request.\r\n\
    ",
    )
    .unwrap();
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
        assert!(
            case.version == 0 || case.version == 1,
            "TestCase.version must 0 or 1"
        );

        let server = serve();

        let mut reply = server.reply();
        for header in case.headers {
            reply = reply.header(header.0, header.1);
        }

        let body_str = match case.body {
            Bd::Known(b) => {
                reply.body(b);
                b
            }
            Bd::Unknown(b) => {
                let (mut tx, body) = hyper::Body::channel();
                tx.try_send_data(b.into()).expect("try_send_data");
                reply.body_stream(body);
                b
            }
        };

        let mut req = connect(server.addr());
        write!(
            req,
            "\
             GET / HTTP/1.{}\r\n\
             Host: example.domain\r\n\
             Connection: close\r\n\
             \r\n\
             ",
            case.version
        )
        .expect("request write");
        let mut body = String::new();
        req.read_to_string(&mut body).unwrap();

        assert_eq!(
            case.expects_chunked,
            has_header(&body, "transfer-encoding:"),
            "expects_chunked, headers = {:?}",
            body
        );

        assert_eq!(
            case.expects_chunked,
            has_header(&body, "chunked\r\n"),
            "expects_chunked, headers = {:?}",
            body
        );

        assert_eq!(
            case.expects_con_len,
            has_header(&body, "content-length:"),
            "expects_con_len, headers = {:?}",
            body
        );

        let n = body.find("\r\n\r\n").unwrap() + 4;

        if case.expects_chunked {
            let len = body.len();
            assert_eq!(
                &body[n + 1..n + 3],
                "\r\n",
                "expected body chunk size header"
            );
            assert_eq!(&body[n + 3..len - 7], body_str, "expected body");
            assert_eq!(
                &body[len - 7..],
                "\r\n0\r\n\r\n",
                "expected body final chunk size header"
            );
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

    #[tokio::test]
    async fn http2_auto_response_with_known_length() {
        use http_body::Body;

        let server = serve();
        let addr_str = format!("http://{}", server.addr());
        server.reply().body("Hello, World!");

        let client = Client::builder()
            .http2_only(true)
            .build_http::<hyper::Body>();
        let uri = addr_str
            .parse::<hyper::Uri>()
            .expect("server addr should parse");

        let res = client.get(uri).await.unwrap();
        assert_eq!(res.headers().get("content-length").unwrap(), "13");
        assert_eq!(res.body().size_hint().exact(), Some(13));
    }

    #[tokio::test]
    async fn http2_auto_response_with_conflicting_lengths() {
        use http_body::Body;

        let server = serve();
        let addr_str = format!("http://{}", server.addr());
        server
            .reply()
            .header("content-length", "10")
            .body("Hello, World!");

        let client = Client::builder()
            .http2_only(true)
            .build_http::<hyper::Body>();
        let uri = addr_str
            .parse::<hyper::Uri>()
            .expect("server addr should parse");

        let res = client.get(uri).await.unwrap();
        assert_eq!(res.headers().get("content-length").unwrap(), "10");
        assert_eq!(res.body().size_hint().exact(), Some(10));
    }
}

#[test]
fn get_chunked_response_with_ka() {
    let foo_bar = b"foo bar baz";
    let foo_bar_chunk = b"\r\nfoo bar baz\r\n0\r\n\r\n";
    let server = serve();
    server
        .reply()
        .header("transfer-encoding", "chunked")
        .body(foo_bar);
    let mut req = connect(server.addr());
    req.write_all(
        b"\
        GET / HTTP/1.1\r\n\
        Host: example.domain\r\n\
        Connection: keep-alive\r\n\
        \r\n\
    ",
    )
    .expect("writing 1");

    read_until(&mut req, |buf| buf.ends_with(foo_bar_chunk)).expect("reading 1");

    // try again!

    let quux = b"zar quux";
    server
        .reply()
        .header("content-length", quux.len().to_string())
        .body(quux);
    req.write_all(
        b"\
        GET /quux HTTP/1.1\r\n\
        Host: example.domain\r\n\
        Connection: close\r\n\
        \r\n\
    ",
    )
    .expect("writing 2");

    read_until(&mut req, |buf| buf.ends_with(quux)).expect("reading 2");
}

#[test]
fn post_with_chunked_body() {
    let server = serve();
    let mut req = connect(server.addr());
    req.write_all(
        b"\
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
    ",
    )
    .unwrap();
    req.read(&mut [0; 256]).unwrap();

    assert_eq!(server.body(), b"qwert");
}

#[test]
fn post_with_incomplete_body() {
    let _ = pretty_env_logger::try_init();
    let server = serve();
    let mut req = connect(server.addr());
    req.write_all(
        b"\
        POST / HTTP/1.1\r\n\
        Host: example.domain\r\n\
        Content-Length: 10\r\n\
        \r\n\
        12345\
    ",
    )
    .expect("write");
    req.shutdown(Shutdown::Write).expect("shutdown write");

    server.body_err();

    req.read(&mut [0; 256]).expect("read");
}

#[test]
fn head_response_can_send_content_length() {
    let _ = pretty_env_logger::try_init();
    let server = serve();
    server.reply().header("content-length", "1024");
    let mut req = connect(server.addr());
    req.write_all(
        b"\
        HEAD / HTTP/1.1\r\n\
        Host: example.domain\r\n\
        Connection: close\r\n\
        \r\n\
    ",
    )
    .unwrap();

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
    server.reply().body(foo_bar);
    let mut req = connect(server.addr());
    req.write_all(
        b"\
        HEAD / HTTP/1.1\r\n\
        Host: example.domain\r\n\
        Connection: close\r\n\
        \r\n\
    ",
    )
    .unwrap();

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
    server
        .reply()
        .status(hyper::StatusCode::NOT_MODIFIED)
        .header("transfer-encoding", "chunked");
    let mut req = connect(server.addr());
    req.write_all(
        b"\
        GET / HTTP/1.1\r\n\
        Host: example.domain\r\n\
        Connection: close\r\n\
        \r\n\
    ",
    )
    .unwrap();

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
    server
        .reply()
        .header("content-length", foo_bar.len().to_string())
        .body(foo_bar);
    let mut req = connect(server.addr());
    req.write_all(
        b"\
        GET / HTTP/1.1\r\n\
        Host: example.domain\r\n\
        \r\n\
    ",
    )
    .expect("writing 1");

    read_until(&mut req, |buf| buf.ends_with(foo_bar)).expect("reading 1");

    // try again!

    let quux = b"zar quux";
    server
        .reply()
        .header("content-length", quux.len().to_string())
        .body(quux);
    req.write_all(
        b"\
        GET /quux HTTP/1.1\r\n\
        Host: example.domain\r\n\
        Connection: close\r\n\
        \r\n\
    ",
    )
    .expect("writing 2");

    read_until(&mut req, |buf| buf.ends_with(quux)).expect("reading 2");
}

#[test]
fn http_10_keep_alive() {
    let foo_bar = b"foo bar baz";
    let server = serve();
    // Response version 1.1 with no keep-alive header will downgrade to 1.0 when served
    server
        .reply()
        .header("content-length", foo_bar.len().to_string())
        .body(foo_bar);
    let mut req = connect(server.addr());
    req.write_all(
        b"\
        GET / HTTP/1.0\r\n\
        Host: example.domain\r\n\
        Connection: keep-alive\r\n\
        \r\n\
    ",
    )
    .expect("writing 1");

    // Connection: keep-alive header should be added when downgrading to a 1.0 response
    let res = read_until(&mut req, |buf| buf.ends_with(foo_bar)).expect("reading 1");

    let sres = s(&res);
    assert!(
        sres.contains("connection: keep-alive\r\n"),
        "HTTP/1.0 response should have sent keep-alive: {:?}",
        sres,
    );

    // try again!

    let quux = b"zar quux";
    server
        .reply()
        .header("content-length", quux.len().to_string())
        .body(quux);
    req.write_all(
        b"\
        GET /quux HTTP/1.0\r\n\
        Host: example.domain\r\n\
        \r\n\
    ",
    )
    .expect("writing 2");

    read_until(&mut req, |buf| buf.ends_with(quux)).expect("reading 2");
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
    )
    .expect("writing 1");

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
    let server = serve_opts().keep_alive(false).serve();
    server
        .reply()
        .header("content-length", foo_bar.len().to_string())
        .body(foo_bar);
    let mut req = connect(server.addr());
    req.write_all(
        b"\
        GET / HTTP/1.1\r\n\
        Host: example.domain\r\n\
        Connection: keep-alive\r\n\
        \r\n\
    ",
    )
    .expect("writing 1");

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
    server
        .reply()
        .header("content-length", foo_bar.len().to_string())
        .header("connection", "close")
        .body(foo_bar);
    let mut req = connect(server.addr());
    req.write_all(
        b"\
        GET / HTTP/1.1\r\n\
        Host: example.domain\r\n\
        Connection: keep-alive\r\n\
        \r\n\
    ",
    )
    .expect("writing 1");

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

    req.write_all(
        b"\
        POST /foo HTTP/1.1\r\n\
        Host: example.domain\r\n\
        Expect: 100-continue\r\n\
        Content-Length: 5\r\n\
        Connection: Close\r\n\
        \r\n\
    ",
    )
    .expect("write 1");

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
fn expect_continue_but_no_body_is_ignored() {
    let server = serve();
    let mut req = connect(server.addr());
    server.reply();

    // no content-length or transfer-encoding means no body!
    req.write_all(
        b"\
        POST /foo HTTP/1.1\r\n\
        Host: example.domain\r\n\
        Expect: 100-continue\r\n\
        Connection: Close\r\n\
        \r\n\
    ",
    )
    .expect("write");

    let expected = "HTTP/1.1 200 OK\r\n";
    let mut resp = String::new();
    req.read_to_string(&mut resp).expect("read");

    assert_eq!(&resp[..expected.len()], expected);
}

#[test]
fn pipeline_disabled() {
    let server = serve();
    let mut req = connect(server.addr());
    server
        .reply()
        .header("content-length", "12")
        .body("Hello World!");
    server
        .reply()
        .header("content-length", "12")
        .body("Hello World!");

    req.write_all(
        b"\
        GET / HTTP/1.1\r\n\
        Host: example.domain\r\n\
        \r\n\
        GET / HTTP/1.1\r\n\
        Host: example.domain\r\n\
        \r\n\
    ",
    )
    .expect("write 1");

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
    let server = serve_opts().pipeline(true).serve();
    let mut req = connect(server.addr());
    server
        .reply()
        .header("content-length", "12")
        .body("Hello World\n");
    server
        .reply()
        .header("content-length", "12")
        .body("Hello World\n");

    req.write_all(
        b"\
        GET / HTTP/1.1\r\n\
        Host: example.domain\r\n\
        \r\n\
        GET / HTTP/1.1\r\n\
        Host: example.domain\r\n\
        Connection: close\r\n\
        \r\n\
    ",
    )
    .expect("write 1");

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
    req.write_all(
        b"\
        GET / HTTP/1.0\r\n\
        \r\n\
    ",
    )
    .unwrap();

    let expected = "HTTP/1.0 200 OK\r\ncontent-length: 0\r\n";
    let mut buf = [0; 256];
    let n = req.read(&mut buf).unwrap();
    assert!(n >= expected.len(), "read: {:?} >= {:?}", n, expected.len());
    assert_eq!(s(&buf[..expected.len()]), expected);
}

#[tokio::test]
async fn disable_keep_alive_mid_request() {
    let mut listener = tcp_bind(&"127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = listener.local_addr().unwrap();

    let (tx1, rx1) = oneshot::channel();
    let (tx2, rx2) = mpsc::channel();

    let child = thread::spawn(move || {
        let mut req = connect(&addr);
        req.write_all(b"GET / HTTP/1.1\r\n").unwrap();
        tx1.send(()).unwrap();
        rx2.recv().unwrap();
        req.write_all(b"Host: localhost\r\n\r\n").unwrap();
        let mut buf = vec![];
        req.read_to_end(&mut buf).unwrap();
    });

    let (socket, _) = listener.accept().await.unwrap();
    let srv = Http::new().serve_connection(socket, HelloWorld);
    future::try_select(srv, rx1)
        .then(|r| match r {
            Ok(Either::Left(_)) => panic!("expected rx first"),
            Ok(Either::Right(((), mut conn))) => {
                Pin::new(&mut conn).graceful_shutdown();
                tx2.send(()).unwrap();
                conn
            }
            Err(Either::Left((e, _))) => panic!("unexpected error {}", e),
            Err(Either::Right((e, _))) => panic!("unexpected error {}", e),
        })
        .await
        .unwrap();

    child.join().unwrap();
}

#[tokio::test]
async fn disable_keep_alive_post_request() {
    let _ = pretty_env_logger::try_init();
    let mut listener = tcp_bind(&"127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = listener.local_addr().unwrap();

    let (tx1, rx1) = oneshot::channel();

    let child = thread::spawn(move || {
        let mut req = connect(&addr);
        req.write_all(
            b"\
            GET / HTTP/1.1\r\n\
            Host: localhost\r\n\
            \r\n\
        ",
        )
        .unwrap();

        read_until(&mut req, |buf| buf.ends_with(HELLO.as_bytes())).expect("reading 1");

        // Connection should get closed *after* tx is sent on
        tx1.send(()).unwrap();

        let nread = req.read(&mut [0u8; 1024]).expect("keep-alive reading");
        assert_eq!(nread, 0);
    });

    let dropped = Dropped::new();
    let dropped2 = dropped.clone();
    let (socket, _) = listener.accept().await.unwrap();
    let transport = DebugStream {
        stream: socket,
        _debug: dropped2,
    };
    let server = Http::new().serve_connection(transport, HelloWorld);
    let fut = future::try_select(server, rx1).then(|r| match r {
        Ok(Either::Left(_)) => panic!("expected rx first"),
        Ok(Either::Right(((), mut conn))) => {
            Pin::new(&mut conn).graceful_shutdown();
            conn
        }
        Err(Either::Left((e, _))) => panic!("unexpected error {}", e),
        Err(Either::Right((e, _))) => panic!("unexpected error {}", e),
    });

    assert!(!dropped.load());
    fut.await.unwrap();
    assert!(dropped.load());
    child.join().unwrap();
}

#[tokio::test]
async fn empty_parse_eof_does_not_return_error() {
    let _ = pretty_env_logger::try_init();
    let mut listener = tcp_bind(&"127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = listener.local_addr().unwrap();

    thread::spawn(move || {
        let _tcp = connect(&addr);
    });

    let (socket, _) = listener.accept().await.unwrap();
    Http::new()
        .serve_connection(socket, HelloWorld)
        .await
        .expect("empty parse eof is ok");
}

#[tokio::test]
async fn nonempty_parse_eof_returns_error() {
    let mut listener = tcp_bind(&"127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = listener.local_addr().unwrap();

    thread::spawn(move || {
        let mut tcp = connect(&addr);
        tcp.write_all(b"GET / HTTP/1.1").unwrap();
    });

    let (socket, _) = listener.accept().await.unwrap();
    Http::new()
        .serve_connection(socket, HelloWorld)
        .await
        .expect_err("partial parse eof is error");
}

#[tokio::test]
async fn http1_allow_half_close() {
    let _ = pretty_env_logger::try_init();
    let mut listener = tcp_bind(&"127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = listener.local_addr().unwrap();

    let t1 = thread::spawn(move || {
        let mut tcp = connect(&addr);
        tcp.write_all(b"GET / HTTP/1.1\r\n\r\n").unwrap();
        tcp.shutdown(::std::net::Shutdown::Write).expect("SHDN_WR");

        let mut buf = [0; 256];
        tcp.read(&mut buf).unwrap();
        let expected = "HTTP/1.1 200 OK\r\n";
        assert_eq!(s(&buf[..expected.len()]), expected);
    });

    let (socket, _) = listener.accept().await.unwrap();
    Http::new()
        .http1_half_close(true)
        .serve_connection(
            socket,
            service_fn(|_| {
                tokio::time::delay_for(Duration::from_millis(500))
                    .map(|_| Ok::<_, hyper::Error>(Response::new(Body::empty())))
            }),
        )
        .await
        .unwrap();

    t1.join().expect("client thread");
}

#[tokio::test]
async fn disconnect_after_reading_request_before_responding() {
    let _ = pretty_env_logger::try_init();
    let mut listener = tcp_bind(&"127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = listener.local_addr().unwrap();

    thread::spawn(move || {
        let mut tcp = connect(&addr);
        tcp.write_all(b"GET / HTTP/1.1\r\n\r\n").unwrap();
    });

    let (socket, _) = listener.accept().await.unwrap();
    Http::new()
        .http1_half_close(false)
        .serve_connection(
            socket,
            service_fn(|_| {
                tokio::time::delay_for(Duration::from_secs(2)).map(
                    |_| -> Result<Response<Body>, hyper::Error> {
                        panic!("response future should have been dropped");
                    },
                )
            }),
        )
        .await
        .expect_err("socket disconnected");
}

#[tokio::test]
async fn returning_1xx_response_is_error() {
    let mut listener = tcp_bind(&"127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = listener.local_addr().unwrap();

    thread::spawn(move || {
        let mut tcp = connect(&addr);
        tcp.write_all(b"GET / HTTP/1.1\r\n\r\n").unwrap();
        let mut buf = [0; 256];
        tcp.read(&mut buf).unwrap();

        let expected = "HTTP/1.1 500 ";
        assert_eq!(s(&buf[..expected.len()]), expected);
    });

    let (socket, _) = listener.accept().await.unwrap();
    Http::new()
        .serve_connection(
            socket,
            service_fn(|_| {
                async move {
                    Ok::<_, hyper::Error>(
                        Response::builder()
                            .status(StatusCode::CONTINUE)
                            .body(Body::empty())
                            .unwrap(),
                    )
                }
            }),
        )
        .await
        .expect_err("1xx status code should error");
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

#[tokio::test]
async fn upgrades() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let _ = pretty_env_logger::try_init();
    let mut listener = tcp_bind(&"127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = oneshot::channel();

    thread::spawn(move || {
        let mut tcp = connect(&addr);
        tcp.write_all(
            b"\
            GET / HTTP/1.1\r\n\
            Upgrade: foobar\r\n\
            Connection: upgrade\r\n\
            \r\n\
            eagerly optimistic\
        ",
        )
        .expect("write 1");
        let mut buf = [0; 256];
        tcp.read(&mut buf).expect("read 1");

        let expected = "HTTP/1.1 101 Switching Protocols\r\n";
        assert_eq!(s(&buf[..expected.len()]), expected);
        let _ = tx.send(());

        let n = tcp.read(&mut buf).expect("read 2");
        assert_eq!(s(&buf[..n]), "foo=bar");
        tcp.write_all(b"bar=foo").expect("write 2");
    });

    let (socket, _) = listener.accept().await.unwrap();
    let conn = Http::new().serve_connection(
        socket,
        service_fn(|_| {
            let res = Response::builder()
                .status(101)
                .header("upgrade", "foobar")
                .body(hyper::Body::empty())
                .unwrap();
            future::ready(Ok::<_, hyper::Error>(res))
        }),
    );

    let parts = conn.without_shutdown().await.unwrap();
    assert_eq!(parts.read_buf, "eagerly optimistic");

    // wait so that we don't write until other side saw 101 response
    rx.await.unwrap();

    let mut io = parts.io;
    io.write_all(b"foo=bar").await.unwrap();
    let mut vec = vec![];
    io.read_to_end(&mut vec).await.unwrap();
    assert_eq!(vec, b"bar=foo");
}

#[tokio::test]
async fn http_connect() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let _ = pretty_env_logger::try_init();
    let mut listener = tcp_bind(&"127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = oneshot::channel();

    thread::spawn(move || {
        let mut tcp = connect(&addr);
        tcp.write_all(
            b"\
            CONNECT localhost:80 HTTP/1.1\r\n\
            \r\n\
            eagerly optimistic\
        ",
        )
        .expect("write 1");
        let mut buf = [0; 256];
        tcp.read(&mut buf).expect("read 1");

        let expected = "HTTP/1.1 200 OK\r\n";
        assert_eq!(s(&buf[..expected.len()]), expected);
        let _ = tx.send(());

        let n = tcp.read(&mut buf).expect("read 2");
        assert_eq!(s(&buf[..n]), "foo=bar");
        tcp.write_all(b"bar=foo").expect("write 2");
    });

    let (socket, _) = listener.accept().await.unwrap();
    let conn = Http::new().serve_connection(
        socket,
        service_fn(|_| {
            let res = Response::builder()
                .status(200)
                .body(hyper::Body::empty())
                .unwrap();
            future::ready(Ok::<_, hyper::Error>(res))
        }),
    );

    let parts = conn.without_shutdown().await.unwrap();
    assert_eq!(parts.read_buf, "eagerly optimistic");

    // wait so that we don't write until other side saw 101 response
    rx.await.unwrap();

    let mut io = parts.io;
    io.write_all(b"foo=bar").await.unwrap();
    let mut vec = vec![];
    io.read_to_end(&mut vec).await.unwrap();
    assert_eq!(vec, b"bar=foo");
}

#[tokio::test]
async fn upgrades_new() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let _ = pretty_env_logger::try_init();
    let mut listener = tcp_bind(&"127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = listener.local_addr().unwrap();
    let (read_101_tx, read_101_rx) = oneshot::channel();

    thread::spawn(move || {
        let mut tcp = connect(&addr);
        tcp.write_all(
            b"\
            GET / HTTP/1.1\r\n\
            Upgrade: foobar\r\n\
            Connection: upgrade\r\n\
            \r\n\
            eagerly optimistic\
        ",
        )
        .expect("write 1");
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
    let svc = service_fn(move |req: Request<Body>| {
        let on_upgrade = req.into_body().on_upgrade();
        let _ = upgrades_tx.send(on_upgrade);
        future::ok::<_, hyper::Error>(
            Response::builder()
                .status(101)
                .header("upgrade", "foobar")
                .body(hyper::Body::empty())
                .unwrap(),
        )
    });

    let (socket, _) = listener.accept().await.unwrap();
    Http::new()
        .serve_connection(socket, svc)
        .with_upgrades()
        .await
        .unwrap();

    let on_upgrade = upgrades_rx.recv().unwrap();

    // wait so that we don't write until other side saw 101 response
    read_101_rx.await.unwrap();

    let upgraded = on_upgrade.await.expect("on_upgrade");
    let parts = upgraded.downcast::<TkTcpStream>().unwrap();
    assert_eq!(parts.read_buf, "eagerly optimistic");

    let mut io = parts.io;
    io.write_all(b"foo=bar").await.unwrap();
    let mut vec = vec![];
    io.read_to_end(&mut vec).await.unwrap();
    assert_eq!(s(&vec), "bar=foo");
}

#[tokio::test]
async fn http_connect_new() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let _ = pretty_env_logger::try_init();
    let mut listener = tcp_bind(&"127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = listener.local_addr().unwrap();
    let (read_200_tx, read_200_rx) = oneshot::channel();

    thread::spawn(move || {
        let mut tcp = connect(&addr);
        tcp.write_all(
            b"\
            CONNECT localhost HTTP/1.1\r\n\
            \r\n\
            eagerly optimistic\
        ",
        )
        .expect("write 1");
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
    let svc = service_fn(move |req: Request<Body>| {
        let on_upgrade = req.into_body().on_upgrade();
        let _ = upgrades_tx.send(on_upgrade);
        future::ok::<_, hyper::Error>(
            Response::builder()
                .status(200)
                .body(hyper::Body::empty())
                .unwrap(),
        )
    });

    let (socket, _) = listener.accept().await.unwrap();
    Http::new()
        .serve_connection(socket, svc)
        .with_upgrades()
        .await
        .unwrap();

    let on_upgrade = upgrades_rx.recv().unwrap();

    // wait so that we don't write until other side saw 200
    read_200_rx.await.unwrap();

    let upgraded = on_upgrade.await.expect("on_upgrade");
    let parts = upgraded.downcast::<TkTcpStream>().unwrap();
    assert_eq!(parts.read_buf, "eagerly optimistic");

    let mut io = parts.io;
    io.write_all(b"foo=bar").await.unwrap();
    let mut vec = vec![];
    io.read_to_end(&mut vec).await.unwrap();
    assert_eq!(s(&vec), "bar=foo");
}

#[tokio::test]
async fn parse_errors_send_4xx_response() {
    let mut listener = tcp_bind(&"127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = listener.local_addr().unwrap();

    thread::spawn(move || {
        let mut tcp = connect(&addr);
        tcp.write_all(b"GE T / HTTP/1.1\r\n\r\n").unwrap();
        let mut buf = [0; 256];
        tcp.read(&mut buf).unwrap();

        let expected = "HTTP/1.1 400 ";
        assert_eq!(s(&buf[..expected.len()]), expected);
    });

    let (socket, _) = listener.accept().await.unwrap();
    Http::new()
        .serve_connection(socket, HelloWorld)
        .await
        .expect_err("HTTP parse error");
}

#[tokio::test]
async fn illegal_request_length_returns_400_response() {
    let mut listener = tcp_bind(&"127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = listener.local_addr().unwrap();

    thread::spawn(move || {
        let mut tcp = connect(&addr);
        tcp.write_all(b"POST / HTTP/1.1\r\nContent-Length: foo\r\n\r\n")
            .unwrap();
        let mut buf = [0; 256];
        tcp.read(&mut buf).unwrap();

        let expected = "HTTP/1.1 400 ";
        assert_eq!(s(&buf[..expected.len()]), expected);
    });

    let (socket, _) = listener.accept().await.unwrap();
    Http::new()
        .serve_connection(socket, HelloWorld)
        .await
        .expect_err("illegal Content-Length should error");
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

#[tokio::test]
async fn max_buf_size() {
    let _ = pretty_env_logger::try_init();
    let mut listener = tcp_bind(&"127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = listener.local_addr().unwrap();

    const MAX: usize = 16_000;

    thread::spawn(move || {
        let mut tcp = connect(&addr);
        tcp.write_all(b"POST /").expect("write 1");
        tcp.write_all(&[b'a'; MAX]).expect("write 2");
        let mut buf = [0; 256];
        tcp.read(&mut buf).expect("read 1");

        let expected = "HTTP/1.1 431 ";
        assert_eq!(s(&buf[..expected.len()]), expected);
    });

    let (socket, _) = listener.accept().await.unwrap();
    Http::new()
        .max_buf_size(MAX)
        .serve_connection(socket, HelloWorld)
        .await
        .expect_err("should TooLarge error");
}

#[cfg(feature = "stream")]
#[test]
fn streaming_body() {
    let _ = pretty_env_logger::try_init();

    // disable keep-alive so we can use read_to_end
    let server = serve_opts().keep_alive(false).serve();

    static S: &[&[u8]] = &[&[b'x'; 1_000] as &[u8]; 1_00] as _;
    let b = futures_util::stream::iter(S.iter()).map(|&s| Ok::<_, hyper::Error>(s));
    let b = hyper::Body::wrap_stream(b);
    server.reply().body_stream(b);

    let mut tcp = connect(server.addr());
    tcp.write_all(b"GET / HTTP/1.1\r\n\r\n").unwrap();
    let mut buf = Vec::new();
    tcp.read_to_end(&mut buf).expect("read 1");

    assert!(
        buf.starts_with(b"HTTP/1.1 200 OK\r\n"),
        "response is 200 OK"
    );
    assert_eq!(buf.len(), 100_789, "full streamed body read");
}

#[test]
fn http1_response_with_http2_version() {
    let server = serve();
    let addr_str = format!("http://{}", server.addr());

    let mut rt = Runtime::new().expect("runtime new");

    server.reply().version(hyper::Version::HTTP_2);

    rt.block_on({
        let client = Client::new();
        let uri = addr_str.parse().expect("server addr should parse");
        client.get(uri)
    })
    .unwrap();
}

#[test]
fn try_h2() {
    let server = serve();
    let addr_str = format!("http://{}", server.addr());

    let mut rt = Runtime::new().expect("runtime new");

    rt.block_on({
        let client = Client::builder()
            .http2_only(true)
            .build_http::<hyper::Body>();
        let uri = addr_str.parse().expect("server addr should parse");

        client.get(uri).map_ok(|_| ()).map_err(|_e| ())
    })
    .unwrap();

    assert_eq!(server.body(), b"");
}

#[test]
fn http1_only() {
    let server = serve_opts().http1_only().serve();
    let addr_str = format!("http://{}", server.addr());

    let mut rt = Runtime::new().expect("runtime new");

    rt.block_on({
        let client = Client::builder()
            .http2_only(true)
            .build_http::<hyper::Body>();
        let uri = addr_str.parse().expect("server addr should parse");
        client.get(uri)
    })
    .unwrap_err();
}

#[tokio::test]
async fn http2_service_error_sends_reset_reason() {
    use std::error::Error;

    let server = serve();
    let addr_str = format!("http://{}", server.addr());

    server
        .reply()
        .error(h2::Error::from(h2::Reason::INADEQUATE_SECURITY));

    let uri = addr_str.parse().expect("server addr should parse");
    dbg!("start");
    let err = dbg!(Client::builder()
        .http2_only(true)
        .build_http::<hyper::Body>()
        .get(uri)
        .await
        .expect_err("client.get"));

    let h2_err = err
        .source()
        .expect("err.source")
        .downcast_ref::<h2::Error>()
        .expect("downcast");

    assert_eq!(h2_err.reason(), Some(h2::Reason::INADEQUATE_SECURITY));
}

#[cfg(feature = "stream")]
#[test]
fn http2_body_user_error_sends_reset_reason() {
    use std::error::Error;
    let server = serve();
    let addr_str = format!("http://{}", server.addr());

    let b = futures_util::stream::once(future::err::<String, _>(h2::Error::from(
        h2::Reason::INADEQUATE_SECURITY,
    )));
    let b = hyper::Body::wrap_stream(b);

    server.reply().body_stream(b);

    let mut rt = Runtime::new().expect("runtime new");

    let err: hyper::Error = rt
        .block_on(async move {
            let client = Client::builder()
                .http2_only(true)
                .build_http::<hyper::Body>();
            let uri = addr_str.parse().expect("server addr should parse");

            let mut res = client.get(uri).await?;

            while let Some(chunk) = res.body_mut().next().await {
                chunk?;
            }
            Ok(())
        })
        .unwrap_err();

    let h2_err = err.source().unwrap().downcast_ref::<h2::Error>().unwrap();

    assert_eq!(h2_err.reason(), Some(h2::Reason::INADEQUATE_SECURITY));
}

struct Http2ReadyErrorSvc;

impl tower_service::Service<Request<Body>> for Http2ReadyErrorSvc {
    type Response = Response<Body>;
    type Error = h2::Error;
    type Future = Box<
        dyn futures_core::Future<Output = Result<Self::Response, Self::Error>>
            + Send
            + Sync
            + Unpin,
    >;

    fn poll_ready(&mut self, _: &mut std::task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Err::<(), _>(h2::Error::from(
            h2::Reason::INADEQUATE_SECURITY,
        )))
    }

    fn call(&mut self, _: hyper::Request<Body>) -> Self::Future {
        unreachable!("poll_ready error should have shutdown conn");
    }
}

#[tokio::test]
#[ignore] // sometimes ECONNRESET wins the race
async fn http2_service_poll_ready_error_sends_goaway() {
    use std::error::Error;

    let _ = pretty_env_logger::try_init();

    let server = hyper::Server::bind(&([127, 0, 0, 1], 0).into())
        .http2_only(true)
        .serve(make_service_fn(|_| {
            async move { Ok::<_, BoxError>(Http2ReadyErrorSvc) }
        }));

    let addr_str = format!("http://{}", server.local_addr());

    tokio::task::spawn(async move {
        server.await.expect("server");
    });

    let uri = addr_str.parse().expect("server addr should parse");
    let err = dbg!(Client::builder()
        .http2_only(true)
        .build_http::<hyper::Body>()
        .get(uri)
        .await
        .expect_err("client.get should fail"));

    // client request should have gotten the specific GOAWAY error...
    let h2_err = err
        .source()
        .expect("source")
        .downcast_ref::<h2::Error>()
        .expect("downcast");

    assert_eq!(h2_err.reason(), Some(h2::Reason::INADEQUATE_SECURITY));
}

#[test]
fn skips_content_length_for_304_responses() {
    let server = serve();
    server
        .reply()
        .status(hyper::StatusCode::NOT_MODIFIED)
        .body("foo");
    let mut req = connect(server.addr());
    req.write_all(
        b"\
        GET / HTTP/1.1\r\n\
        Host: example.domain\r\n\
        Connection: close\r\n\
        \r\n\
    ",
    )
    .unwrap();

    let mut response = String::new();
    req.read_to_string(&mut response).unwrap();
    assert!(!response.contains("content-length:"));
}

#[test]
fn skips_content_length_and_body_for_304_responses() {
    let server = serve();
    server
        .reply()
        .status(hyper::StatusCode::NOT_MODIFIED)
        .body("foo");
    let mut req = connect(server.addr());
    req.write_all(
        b"\
        GET / HTTP/1.1\r\n\
        Host: example.domain\r\n\
        Connection: close\r\n\
        \r\n\
    ",
    )
    .unwrap();

    let mut response = String::new();
    req.read_to_string(&mut response).unwrap();
    assert!(!response.contains("content-length:"));
    let mut lines = response.lines();
    assert_eq!(lines.next(), Some("HTTP/1.1 304 Not Modified"));

    let mut lines = lines.skip_while(|line| !line.is_empty());
    assert_eq!(lines.next(), Some(""));
    assert_eq!(lines.next(), None);
}
// -------------------------------------------------
// the Server that is used to run all the tests with
// -------------------------------------------------

struct Serve {
    addr: SocketAddr,
    msg_rx: mpsc::Receiver<Msg>,
    reply_tx: Mutex<spmc::Sender<Reply>>,
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
                }
                Ok(Msg::Error(e)) => return Err(e),
                Ok(Msg::End) => break,
                Err(e) => panic!("expected body, found: {:?}", e),
            }
        }
        Ok(buf)
    }

    fn reply(&self) -> ReplyBuilder<'_> {
        ReplyBuilder { tx: &self.reply_tx }
    }
}

type BoxError = Box<dyn std::error::Error + Send + Sync>;

struct ReplyBuilder<'a> {
    tx: &'a Mutex<spmc::Sender<Reply>>,
}

impl<'a> ReplyBuilder<'a> {
    fn status(self, status: hyper::StatusCode) -> Self {
        self.tx.lock().unwrap().send(Reply::Status(status)).unwrap();
        self
    }

    fn version(self, version: hyper::Version) -> Self {
        self.tx
            .lock()
            .unwrap()
            .send(Reply::Version(version))
            .unwrap();
        self
    }

    fn header<V: AsRef<str>>(self, name: &str, value: V) -> Self {
        let name = HeaderName::from_bytes(name.as_bytes()).expect("header name");
        let value = HeaderValue::from_str(value.as_ref()).expect("header value");
        self.tx
            .lock()
            .unwrap()
            .send(Reply::Header(name, value))
            .unwrap();
        self
    }

    fn body<T: AsRef<[u8]>>(self, body: T) {
        self.tx
            .lock()
            .unwrap()
            .send(Reply::Body(body.as_ref().to_vec().into()))
            .unwrap();
    }

    fn body_stream(self, body: Body) {
        self.tx.lock().unwrap().send(Reply::Body(body)).unwrap();
    }

    #[allow(dead_code)]
    fn error<E: Into<BoxError>>(self, err: E) {
        self.tx
            .lock()
            .unwrap()
            .send(Reply::Error(err.into()))
            .unwrap();
    }
}

impl<'a> Drop for ReplyBuilder<'a> {
    fn drop(&mut self) {
        if let Ok(mut tx) = self.tx.lock() {
            let _ = tx.send(Reply::End);
        }
    }
}

impl Drop for Serve {
    fn drop(&mut self) {
        drop(self.shutdown_signal.take());
        drop(self.thread.take());
        /*
        let r = self.thread.take().unwrap().join();
        if let Err(ref e) = r {
            println!("{:?}", e);
        }
        r.unwrap();
        */
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
    Error(BoxError),
    End,
}

#[derive(Debug)]
enum Msg {
    Chunk(Vec<u8>),
    Error(hyper::Error),
    End,
}

impl tower_service::Service<Request<Body>> for TestService {
    type Response = Response<Body>;
    type Error = BoxError;
    type Future = Pin<Box<dyn Future<Output = Result<Response<Body>, BoxError>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Ok(()).into()
    }

    fn call(&mut self, mut req: Request<Body>) -> Self::Future {
        let tx = self.tx.clone();
        let replies = self.reply.clone();

        Box::pin(async move {
            while let Some(chunk) = req.data().await {
                match chunk {
                    Ok(chunk) => {
                        tx.send(Msg::Chunk(chunk.to_vec())).unwrap();
                    }
                    Err(err) => {
                        tx.send(Msg::Error(err)).unwrap();
                        return Err("req body error".into());
                    }
                }
            }

            tx.send(Msg::End).unwrap();

            TestService::build_reply(replies)
        })
    }
}

impl TestService {
    fn build_reply(replies: spmc::Receiver<Reply>) -> Result<Response<Body>, BoxError> {
        let mut res = Response::new(Body::empty());
        while let Ok(reply) = replies.try_recv() {
            match reply {
                Reply::Status(s) => {
                    *res.status_mut() = s;
                }
                Reply::Version(v) => {
                    *res.version_mut() = v;
                }
                Reply::Header(name, value) => {
                    res.headers_mut().insert(name, value);
                }
                Reply::Body(body) => {
                    *res.body_mut() = body;
                }
                Reply::Error(err) => return Err(err),
                Reply::End => break,
            }
        }
        Ok(res)
    }
}

const HELLO: &str = "hello";

struct HelloWorld;

impl tower_service::Service<Request<Body>> for HelloWorld {
    type Response = Response<Body>;
    type Error = hyper::Error;
    type Future = future::Ready<Result<Response<Body>, Self::Error>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Ok(()).into()
    }

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
        let thread = thread::Builder::new()
            .name(thread_name)
            .spawn(move || {
                let mut rt = tokio::runtime::Builder::new()
                    .enable_io()
                    .enable_time()
                    .basic_scheduler()
                    .build()
                    .expect("rt new");

                rt.block_on(async move {
                    let service = make_service_fn(|_| {
                        let msg_tx = msg_tx.clone();
                        let reply_rx = reply_rx.clone();
                        future::ok::<_, BoxError>(TestService {
                            tx: msg_tx,
                            reply: reply_rx,
                        })
                    });

                    let server = Server::bind(&addr)
                        .http1_only(options.http1_only)
                        .http1_keepalive(options.keep_alive)
                        .http1_pipeline_flush(options.pipeline)
                        .serve(service);

                    addr_tx.send(server.local_addr()).expect("server addr tx");

                    server
                        .with_graceful_shutdown(async {
                            let _ = shutdown_rx.await;
                        })
                        .await
                })
                .expect("serve()");
            })
            .expect("thread spawn");

        let addr = addr_rx.recv().expect("server addr rx");

        Serve {
            msg_rx,
            reply_tx: Mutex::new(reply_tx),
            addr,
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
    TcpListener::from_std(std_listener)
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
                "read_until buffer filled",
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

impl<T: Unpin, D> Unpin for DebugStream<T, D> {}

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

impl<T: AsyncWrite + Unpin, D> AsyncWrite for DebugStream<T, D> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        Pin::new(&mut self.stream).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        Pin::new(&mut self.stream).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), io::Error>> {
        Pin::new(&mut self.stream).poll_shutdown(cx)
    }
}

impl<T: AsyncRead + Unpin, D: Unpin> AsyncRead for DebugStream<T, D> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.stream).poll_read(cx, buf)
    }
}

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
