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
use std::sync::{Arc, Mutex};
use std::net::{TcpListener as StdTcpListener};
use std::thread;
use std::time::Duration;

use futures::{Future, Stream};
use futures::future::{self, FutureResult, Either};
use futures::sync::oneshot;
use futures_timer::Delay;
use http::header::{HeaderName, HeaderValue};
//use net2::TcpBuilder;
use tokio::net::TcpListener;
use tokio::runtime::Runtime;
use tokio::reactor::Handle;
use tokio_io::{AsyncRead, AsyncWrite};


use hyper::{Body, Request, Response, StatusCode};
use hyper::server::{Http, Service, NewService, service_fn};

fn tcp_bind(addr: &SocketAddr, handle: &Handle) -> ::tokio::io::Result<TcpListener> {
    let std_listener = StdTcpListener::bind(addr).unwrap();
    TcpListener::from_std(std_listener, handle)
}

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

#[test]
fn get_implicitly_empty() {
    // See https://github.com/hyperium/hyper/issues/1373
    let runtime = Runtime::new().unwrap();
    let listener = tcp_bind(&"127.0.0.1:0".parse().unwrap(), &runtime.reactor()).unwrap();
    let addr = listener.local_addr().unwrap();

    thread::spawn(move || {
        let mut tcp = connect(&addr);
        tcp.write_all(b"\
            GET / HTTP/1.1\r\n\
            Host: example.domain\r\n\
            \r\n\
        ").unwrap();
    });

    let fut = listener.incoming()
        .into_future()
        .map_err(|_| unreachable!())
        .and_then(|(item, _incoming)| {
            let socket = item.unwrap();
            Http::<hyper::Chunk>::new().serve_connection(socket, GetImplicitlyEmpty)
        });

    fut.wait().unwrap();

    struct GetImplicitlyEmpty;

    impl Service for GetImplicitlyEmpty {
        type Request = Request<Body>;
        type Response = Response<Body>;
        type Error = hyper::Error;
        type Future = Box<Future<Item=Self::Response, Error=Self::Error> + Send>;

        fn call(&self, req: Request<Body>) -> Self::Future {
            Box::new(req.into_body()
                .into_stream()
                .concat2()
                .map(|buf| {
                    assert!(buf.is_empty());
                    Response::new(Body::empty())
                }))
        }
    }
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
    fn get_fixed_response_known() {
        run_test(TestCase {
            version: 1,
            headers: &[("content-length", "11")],
            body: Bd::Known("foo bar baz"),
            expects_chunked: false,
            expects_con_len: true,
        });
    }

    #[test]
    fn get_fixed_response_unknown() {
        run_test(TestCase {
            version: 1,
            headers: &[("content-length", "11")],
            body: Bd::Unknown("foo bar baz"),
            expects_chunked: false,
            expects_con_len: true,
        });
    }

    #[test]
    fn get_chunked_response_known() {
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
    fn get_chunked_response_unknown() {
        run_test(TestCase {
            version: 1,
            headers: &[("transfer-encoding", "chunked")],
            body: Bd::Unknown("foo bar baz"),
            expects_chunked: true,
            expects_con_len: false,
        });
    }

    #[test]
    fn get_chunked_response_trumps_length() {
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
    fn get_auto_response_with_entity_unknown_length() {
        run_test(TestCase {
            version: 1,
            // no headers means trying to guess from Entity
            headers: &[],
            body: Bd::Unknown("foo bar baz"),
            expects_chunked: true,
            expects_con_len: false,
        });
    }

    #[test]
    fn get_auto_response_with_entity_known_length() {
        run_test(TestCase {
            version: 1,
            // no headers means trying to guess from Entity
            headers: &[],
            body: Bd::Known("foo bar baz"),
            expects_chunked: false,
            expects_con_len: true,
        });
    }


    #[test]
    fn http_10_get_auto_response_with_entity_unknown_length() {
        run_test(TestCase {
            version: 0,
            // no headers means trying to guess from Entity
            headers: &[],
            body: Bd::Unknown("foo bar baz"),
            expects_chunked: false,
            expects_con_len: false,
        });
    }


    #[test]
    fn http_10_get_chunked_response() {
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

    let mut buf = [0; 1024 * 4];
    let mut ntotal = 0;
    loop {
        let n = req.read(&mut buf[ntotal..]).expect("reading 1");
        ntotal = ntotal + n;
        assert!(ntotal < buf.len());
        if &buf[ntotal - foo_bar_chunk.len()..ntotal] == foo_bar_chunk {
            break;
        }
    }


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

    let mut buf = [0; 1024 * 8];
    loop {
        let n = req.read(&mut buf[..]).expect("reading 2");
        assert!(n > 0, "n = {}", n);
        if n < buf.len() && n > 0  {
            if &buf[n - quux.len()..n] == quux {
                break;
            }
        }
    }
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
    extern crate pretty_env_logger;
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
    extern crate pretty_env_logger;
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
fn response_does_not_set_chunked_if_body_not_allowed() {
    extern crate pretty_env_logger;
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

    let mut buf = [0; 1024 * 8];
    loop {
        let n = req.read(&mut buf[..]).expect("reading 1");
        if n < buf.len() {
            if &buf[n - foo_bar.len()..n] == foo_bar {
                break;
            }
        }
    }

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

    let mut buf = [0; 1024 * 8];
    loop {
        let n = req.read(&mut buf[..]).expect("reading 2");
        assert!(n > 0, "n = {}", n);
        if n < buf.len() {
            if &buf[n - quux.len()..n] == quux {
                break;
            }
        }
    }
}

#[test]
fn http_10_keep_alive() {
    let foo_bar = b"foo bar baz";
    let server = serve();
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

    let mut buf = [0; 1024 * 8];
    loop {
        let n = req.read(&mut buf[..]).expect("reading 1");
        if n < buf.len() {
            if &buf[n - foo_bar.len()..n] == foo_bar {
                break;
            }
        }
    }

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

    let mut buf = [0; 1024 * 8];
    loop {
        let n = req.read(&mut buf[..]).expect("reading 2");
        assert!(n > 0, "n = {}", n);
        if n < buf.len() {
            if &buf[n - quux.len()..n] == quux {
                break;
            }
        }
    }
}

#[test]
fn disable_keep_alive() {
    let foo_bar = b"foo bar baz";
    let server = serve_with_options(ServeOptions {
        keep_alive_disabled: true,
        .. Default::default()
    });
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

    let mut buf = [0; 1024 * 8];
    loop {
        let n = req.read(&mut buf[..]).expect("reading 1");
        if n < buf.len() {
            if &buf[n - foo_bar.len()..n] == foo_bar {
                break;
            } else {
            }
        }
    }

    // try again!

    let quux = b"zar quux";
    server.reply()
        .header("content-length", quux.len().to_string())
        .body(quux);

    // the write can possibly succeed, since it fills the kernel buffer on the first write
    let _ = req.write_all(b"\
        GET /quux HTTP/1.1\r\n\
        Host: example.domain\r\n\
        Connection: close\r\n\
        \r\n\
    ");

    let mut buf = [0; 1024 * 8];
    match req.read(&mut buf[..]) {
        // Ok(0) means EOF, so a proper shutdown
        // Err(_) could mean ConnReset or something, also fine
        Ok(0) |
        Err(_) => {}
        Ok(n) => {
            panic!("read {} bytes on a disabled keep-alive socket", n);
        }
    }
}

#[test]
fn expect_continue() {
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
    let server = serve_with_options(ServeOptions {
        pipeline: true,
        .. Default::default()
    });
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
    let runtime = Runtime::new().unwrap();
    let listener = tcp_bind(&"127.0.0.1:0".parse().unwrap(), &runtime.reactor()).unwrap();
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
            Http::<hyper::Chunk>::new().serve_connection(socket, HelloWorld)
                .select2(rx1)
                .then(|r| {
                    match r {
                        Ok(Either::A(_)) => panic!("expected rx first"),
                        Ok(Either::B(((), mut conn))) => {
                            conn.disable_keep_alive();
                            tx2.send(()).unwrap();
                            conn
                        }
                        Err(Either::A((e, _))) => panic!("unexpected error {}", e),
                        Err(Either::B((e, _))) => panic!("unexpected error {}", e),
                    }
                })
        });

    fut.wait().unwrap();
    child.join().unwrap();
}

#[test]
fn disable_keep_alive_post_request() {
    let _ = pretty_env_logger::try_init();
    let runtime = Runtime::new().unwrap();
    let listener = tcp_bind(&"127.0.0.1:0".parse().unwrap(), &runtime.reactor()).unwrap();
    let addr = listener.local_addr().unwrap();

    let (tx1, rx1) = oneshot::channel();

    let child = thread::spawn(move || {
        let mut req = connect(&addr);
        req.write_all(b"\
            GET / HTTP/1.1\r\n\
            Host: localhost\r\n\
            \r\n\
        ").unwrap();

        let mut buf = [0; 1024 * 8];
        loop {
            let n = req.read(&mut buf).expect("reading 1");
            if &buf[n - HELLO.len()..n] == HELLO.as_bytes() {
                break;
            }
            if n == 0 {
                panic!("unexpected eof");
            }
        }

        tx1.send(()).unwrap();

        let nread = req.read(&mut buf).expect("keep-alive reading");
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
            Http::<hyper::Chunk>::new().serve_connection(transport, HelloWorld)
                .select2(rx1)
                .then(|r| {
                    match r {
                        Ok(Either::A(_)) => panic!("expected rx first"),
                        Ok(Either::B(((), mut conn))) => {
                            conn.disable_keep_alive();
                            conn
                        }
                        Err(Either::A((e, _))) => panic!("unexpected error {}", e),
                        Err(Either::B((e, _))) => panic!("unexpected error {}", e),
                    }
                })
        });

    assert!(!dropped.load());
    fut.wait().unwrap();
    // we must poll the Core one more time in order for Windows to drop
    // the read-blocked socket.
    //
    // See https://github.com/carllerche/mio/issues/776
    let timeout = Delay::new(Duration::from_millis(10));
    timeout.wait().unwrap();
    assert!(dropped.load());
    child.join().unwrap();
}

#[test]
fn empty_parse_eof_does_not_return_error() {
    let runtime = Runtime::new().unwrap();
    let listener = tcp_bind(&"127.0.0.1:0".parse().unwrap(), &runtime.reactor()).unwrap();
    let addr = listener.local_addr().unwrap();

    thread::spawn(move || {
        let _tcp = connect(&addr);
    });

    let fut = listener.incoming()
        .into_future()
        .map_err(|_| unreachable!())
        .and_then(|(item, _incoming)| {
            let socket = item.unwrap();
            Http::<hyper::Chunk>::new().serve_connection(socket, HelloWorld)
        });

    fut.wait().unwrap();
}

#[test]
fn nonempty_parse_eof_returns_error() {
    let runtime = Runtime::new().unwrap();
    let listener = tcp_bind(&"127.0.0.1:0".parse().unwrap(), &runtime.reactor()).unwrap();
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
            Http::<hyper::Chunk>::new().serve_connection(socket, HelloWorld)
                .map(|_| ())
        });

    fut.wait().unwrap_err();
}

#[test]
fn returning_1xx_response_is_error() {
    let runtime = Runtime::new().unwrap();
    let listener = tcp_bind(&"127.0.0.1:0".parse().unwrap(), &runtime.reactor()).unwrap();
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
            Http::<hyper::Chunk>::new()
                .serve_connection(socket, service_fn(|_| {
                    Ok::<_, hyper::Error>(Response::builder()
                        .status(StatusCode::CONTINUE)
                        .body(Body::empty())
                        .unwrap())
                }))
                .map(|_| ())
        });

    fut.wait().unwrap_err();
}

#[test]
fn upgrades() {
    use tokio_io::io::{read_to_end, write_all};
    let _ = pretty_env_logger::try_init();
    let runtime = Runtime::new().unwrap();
    let listener = tcp_bind(&"127.0.0.1:0".parse().unwrap(), &runtime.reactor()).unwrap();
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
            let conn = Http::<hyper::Chunk>::new()
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

    let conn = fut.wait().unwrap();

    // wait so that we don't write until other side saw 101 response
    rx.wait().unwrap();

    let parts = conn.into_parts();
    let io = parts.io;
    assert_eq!(parts.read_buf, "eagerly optimistic");

    let io = write_all(io, b"foo=bar").wait().unwrap().0;
    let vec = read_to_end(io, vec![]).wait().unwrap().1;
    assert_eq!(vec, b"bar=foo");
}

#[test]
fn parse_errors_send_4xx_response() {
    let runtime = Runtime::new().unwrap();
    let listener = tcp_bind(&"127.0.0.1:0".parse().unwrap(), &runtime.reactor()).unwrap();
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
            Http::<hyper::Chunk>::new()
                .serve_connection(socket, HelloWorld)
                .map(|_| ())
        });

    fut.wait().unwrap_err();
}

#[test]
fn illegal_request_length_returns_400_response() {
    let runtime = Runtime::new().unwrap();
    let listener = tcp_bind(&"127.0.0.1:0".parse().unwrap(), &runtime.reactor()).unwrap();
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
            Http::<hyper::Chunk>::new()
                .serve_connection(socket, HelloWorld)
                .map(|_| ())
        });

    fut.wait().unwrap_err();
}

#[test]
fn max_buf_size() {
    let _ = pretty_env_logger::try_init();
    let runtime = Runtime::new().unwrap();
    let listener = tcp_bind(&"127.0.0.1:0".parse().unwrap(), &runtime.reactor()).unwrap();
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
            Http::<hyper::Chunk>::new()
                .max_buf_size(MAX)
                .serve_connection(socket, HelloWorld)
                .map(|_| ())
        });

    fut.wait().unwrap_err();
}

#[test]
fn streaming_body() {
    let _ = pretty_env_logger::try_init();
    let runtime = Runtime::new().unwrap();
    let listener = tcp_bind(&"127.0.0.1:0".parse().unwrap(), &runtime.reactor()).unwrap();
    let addr = listener.local_addr().unwrap();

    let (tx, rx) = oneshot::channel();
    thread::spawn(move || {
        let mut tcp = connect(&addr);
        tcp.write_all(b"GET / HTTP/1.1\r\n\r\n").unwrap();
        let mut buf = [0; 8192];
        let mut sum = tcp.read(&mut buf).expect("read 1");

        let expected = "HTTP/1.1 200 ";
        assert_eq!(s(&buf[..expected.len()]), expected);

        loop {
            let n = tcp.read(&mut buf).expect("read loop");
            sum += n;
            if n == 0 {
                break;
            }
        }
        assert_eq!(sum, 100_789);
        let _ = tx.send(());
    });

    let rx = rx.map_err(|_| panic!("thread panicked"));

    let fut = listener.incoming()
        .into_future()
        .map_err(|_| unreachable!())
        .and_then(|(item, _incoming)| {
            let socket = item.unwrap();
            Http::<hyper::Chunk>::new()
                .keep_alive(false)
                .serve_connection(socket, service_fn(|_| {
                    static S: &'static [&'static [u8]] = &[&[b'x'; 1_000] as &[u8]; 1_00] as _;
                    let b = ::futures::stream::iter_ok::<_, String>(S.into_iter())
                        .map(|&s| s);
                    let b = hyper::Body::wrap_stream(b);
                    Ok::<_, hyper::Error>(Response::new(b))
                }))
                .map(|_| ())
        });

    fut.join(rx).wait().unwrap();
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
    tx: Arc<Mutex<mpsc::Sender<Msg>>>,
    reply: spmc::Receiver<Reply>,
    _timeout: Option<Duration>,
}

#[derive(Debug)]
enum Reply {
    Status(hyper::StatusCode),
    Header(HeaderName, HeaderValue),
    Body(hyper::Body),
    End,
}

#[derive(Debug)]
enum Msg {
    //Head(Request),
    Chunk(Vec<u8>),
    Error(hyper::Error),
    End,
}

impl NewService for TestService {
    type Request = Request<Body>;
    type Response = Response<Body>;
    type Error = hyper::Error;

    type Instance = TestService;

    fn new_service(&self) -> std::io::Result<TestService> {
        Ok(self.clone())
    }
}

impl Service for TestService {
    type Request = Request<Body>;
    type Response = Response<Body>;
    type Error = hyper::Error;
    type Future = Box<Future<Item=Response<Body>, Error=hyper::Error> + Send>;
    fn call(&self, req: Request<Body>) -> Self::Future {
        let tx1 = self.tx.clone();
        let tx2 = self.tx.clone();

        let replies = self.reply.clone();
        Box::new(req.into_body().into_stream().for_each(move |chunk| {
            tx1.lock().unwrap().send(Msg::Chunk(chunk.to_vec())).unwrap();
            Ok(())
        }).then(move |result| {
            let msg = match result {
                Ok(()) => Msg::End,
                Err(e) => Msg::Error(e),
            };
            tx2.lock().unwrap().send(msg).unwrap();
            Ok(())
        }).map(move |_| {
            let mut res = Response::new(Body::empty());
            while let Ok(reply) = replies.try_recv() {
                match reply {
                    Reply::Status(s) => {
                        *res.status_mut() = s;
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
        }))
    }

}

const HELLO: &'static str = "hello";

struct HelloWorld;

impl Service for HelloWorld {
    type Request = Request<Body>;
    type Response = Response<Body>;
    type Error = hyper::Error;
    type Future = FutureResult<Self::Response, Self::Error>;

    fn call(&self, _req: Request<Body>) -> Self::Future {
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
    serve_with_options(Default::default())
}

struct ServeOptions {
    keep_alive_disabled: bool,
    pipeline: bool,
    timeout: Option<Duration>,
}

impl Default for ServeOptions {
    fn default() -> Self {
        ServeOptions {
            keep_alive_disabled: false,
            pipeline: false,
            timeout: None,
        }
    }
}

fn serve_with_options(options: ServeOptions) -> Serve {
    let _ = pretty_env_logger::try_init();

    let (addr_tx, addr_rx) = mpsc::channel();
    let (msg_tx, msg_rx) = mpsc::channel();
    let (reply_tx, reply_rx) = spmc::channel();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();

    let addr = "127.0.0.1:0".parse().unwrap();

    let keep_alive = !options.keep_alive_disabled;
    let pipeline = options.pipeline;
    let dur = options.timeout;

    let thread_name = format!("test-server-{:?}", dur);
    let thread = thread::Builder::new().name(thread_name).spawn(move || {
        tokio::run(::futures::future::lazy(move || {
            let srv = Http::new()
                .keep_alive(keep_alive)
                .pipeline(pipeline)
                .bind(&addr, TestService {
                    tx: Arc::new(Mutex::new(msg_tx.clone())),
                    _timeout: dur,
                    reply: reply_rx,
                }).unwrap();
            addr_tx.send(srv.local_addr().unwrap()).unwrap();
            srv.run_until(shutdown_rx.then(|_| Ok(())))
                .map_err(|err| println!("error {}", err))
        }))
    }).unwrap();

    let addr = addr_rx.recv().unwrap();

    Serve {
        msg_rx: msg_rx,
        reply_tx: reply_tx,
        addr: addr,
        shutdown_signal: Some(shutdown_tx),
        thread: Some(thread),
    }
}

fn s(buf: &[u8]) -> &str {
    ::std::str::from_utf8(buf).unwrap()
}

fn has_header(msg: &str, name: &str) -> bool {
    let n = msg.find("\r\n\r\n").unwrap_or(msg.len());

    msg[..n].contains(name)
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

