#![deny(warnings)]
extern crate hyper;

use std::io::{self, Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;
use std::time::Duration;

use hyper::client::{Handler, Request, Response, HttpConnector};
use hyper::{Method, StatusCode, Next, Encoder, Decoder};
use hyper::header::Headers;
use hyper::net::HttpStream;

fn s(bytes: &[u8]) -> &str {
    ::std::str::from_utf8(bytes.as_ref()).unwrap()
}

#[derive(Debug)]
struct TestHandler {
    opts: Opts,
    tx: mpsc::Sender<Msg>
}

impl TestHandler {
    fn new(opts: Opts) -> (TestHandler, mpsc::Receiver<Msg>) {
        let (tx, rx) = mpsc::channel();
        (TestHandler {
            opts: opts,
            tx: tx
        }, rx)
    }
}

#[derive(Debug)]
enum Msg {
    Head(Response),
    Chunk(Vec<u8>),
    Error(hyper::Error),
}

fn read(opts: &Opts) -> Next {
    if let Some(timeout) = opts.read_timeout {
        Next::read().timeout(timeout)
    } else {
        Next::read()
    }
}

impl Handler<HttpStream> for TestHandler {
    fn on_request(&mut self, req: &mut Request) -> Next {
        req.set_method(self.opts.method.clone());
        req.headers_mut().extend(self.opts.headers.iter());
        if self.opts.body.is_some() {
            Next::write()
        } else {
            read(&self.opts)
        }
    }

    fn on_request_writable(&mut self, encoder: &mut Encoder<HttpStream>) -> Next {
        if let Some(ref mut body) = self.opts.body {
            let n = encoder.write(body).unwrap();
            *body = &body[n..];

            if !body.is_empty() {
                return Next::write()
            }
        }
        encoder.close();
        read(&self.opts)
    }

    fn on_response(&mut self, res: Response) -> Next {
        use hyper::header;
        // server responses can include a body until eof, if not size is specified
        let mut has_body = true;
        if let Some(len) = res.headers().get::<header::ContentLength>() {
            if **len == 0 {
                has_body = false;
            }
        }
        self.tx.send(Msg::Head(res)).unwrap();
        if has_body {
            read(&self.opts)
        } else {
            Next::end()
        }
    }

    fn on_response_readable(&mut self, decoder: &mut Decoder<HttpStream>) -> Next {
        let mut v = vec![0; 512];
        match decoder.read(&mut v) {
            Ok(n) => {
                v.truncate(n);
                self.tx.send(Msg::Chunk(v)).unwrap();
                if n == 0 {
                    Next::end()
                } else {
                    read(&self.opts)
                }
            },
            Err(e) => match e.kind() {
                io::ErrorKind::WouldBlock => read(&self.opts),
                _ => panic!("io read error: {:?}", e)
            }
        }
    }

    fn on_error(&mut self, err: hyper::Error) -> Next {
        self.tx.send(Msg::Error(err)).unwrap();
        Next::remove()
    }
}

struct Client {
    client: Option<hyper::Client<TestHandler>>,
}

#[derive(Debug)]
struct Opts {
    body: Option<&'static [u8]>,
    method: Method,
    headers: Headers,
    read_timeout: Option<Duration>,
}

impl Default for Opts {
    fn default() -> Opts {
        Opts {
            body: None,
            method: Method::Get,
            headers: Headers::new(),
            read_timeout: None,
        }
    }
}

fn opts() -> Opts {
    Opts::default()
}

impl Opts {
    fn method(mut self, method: Method) -> Opts {
        self.method = method;
        self
    }

    fn header<H: ::hyper::header::Header>(mut self, header: H) -> Opts {
        self.headers.set(header);
        self
    }

    fn body(mut self, body: Option<&'static [u8]>) -> Opts {
        self.body = body;
        self
    }

    fn read_timeout(mut self, timeout: Duration) -> Opts {
        self.read_timeout = Some(timeout);
        self
    }
}

impl Client {
    fn request<U>(&self, url: U, opts: Opts) -> mpsc::Receiver<Msg>
    where U: AsRef<str> {
        let (handler, rx) = TestHandler::new(opts);
        self.client.as_ref().unwrap()
            .request(url.as_ref().parse().unwrap(), handler).unwrap();
        rx
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        self.client.take().map(|c| c.close());
    }
}

fn client() -> Client {
    let c = hyper::Client::<TestHandler>::configure()
        .connector(HttpConnector::default())
        .build().unwrap();
    Client {
        client: Some(c),
    }
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
            let client = client();
            let opts = opts()
                .method(Method::$client_method)
                .body($request_body);
            $(
                let opts = opts.header($request_headers);
            )*
            let res = client.request(format!($client_url, addr=addr), opts);

            let mut inc = server.accept().unwrap().0;
            inc.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            inc.set_write_timeout(Some(Duration::from_secs(5))).unwrap();
            let expected = format!($server_expected, addr=addr);
            let mut buf = [0; 4096];
            let mut n = 0;
            while n < buf.len() && n < expected.len() {
                n += inc.read(&mut buf[n..]).unwrap();
            }
            assert_eq!(s(&buf[..n]), expected);

            inc.write_all($server_reply.as_ref()).unwrap();

            if let Msg::Head(head) = res.recv().unwrap() {
                assert_eq!(head.status(), &StatusCode::$client_status);
                $(
                    assert_eq!(head.headers().get(), Some(&$response_headers));
                )*
            } else {
                panic!("we lost the head!");
            }
            //drop(inc);

            assert!(res.recv().is_err());
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
