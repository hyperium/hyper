#![deny(warnings)]
extern crate hyper;

use std::io::{self, Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;
use std::time::Duration;

use hyper::client::{Handler, Request, Response, HttpConnector};
use hyper::header;
use hyper::{Method, StatusCode, Next, Encoder, Decoder};
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
        read(&self.opts)
    }

    fn on_request_writable(&mut self, _encoder: &mut Encoder<HttpStream>) -> Next {
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
    method: Method,
    read_timeout: Option<Duration>,
}

impl Default for Opts {
    fn default() -> Opts {
        Opts {
            method: Method::Get,
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


#[test]
fn client_get() {
    let server = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = server.local_addr().unwrap();
    let client = client();
    let res = client.request(format!("http://{}/", addr), opts().method(Method::Get));

    let mut inc = server.accept().unwrap().0;
    inc.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    inc.set_write_timeout(Some(Duration::from_secs(5))).unwrap();
    let mut buf = [0; 4096];
    let n = inc.read(&mut buf).unwrap();
    let expected = format!("GET / HTTP/1.1\r\nHost: {}\r\n\r\n", addr);
    assert_eq!(s(&buf[..n]), expected);

    inc.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n").unwrap();

    if let Msg::Head(head) = res.recv().unwrap() {
        assert_eq!(head.status(), &StatusCode::Ok);
        assert_eq!(head.headers().get(), Some(&header::ContentLength(0)));
    } else {
        panic!("we lost the head!");
    }
    //drop(inc);

    assert!(res.recv().is_err());
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
