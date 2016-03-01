extern crate hyper;

use std::io::{self, Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;
use std::time::Duration;

use hyper::client::{Handler, Request, Response};
use hyper::header;
use hyper::{Method, StatusCode, Next, Encoder, Decoder};
use hyper::net::HttpStream;

fn s(bytes: &[u8]) -> &str {
    ::std::str::from_utf8(bytes.as_ref()).unwrap()
}

struct TestHandler {
    method: Method,
    tx: mpsc::Sender<Msg>
}

impl TestHandler {
    fn new(method: Method) -> (TestHandler, mpsc::Receiver<Msg>) {
        let (tx, rx) = mpsc::channel();
        (TestHandler {
            method: method,
            tx: tx
        }, rx)
    }
}

enum Msg {
    Head(Response),
    Chunk(Vec<u8>)
}

impl Handler<HttpStream> for TestHandler {
    fn on_request(&mut self, req: &mut Request) -> Next {
        req.set_method(self.method.clone());
        Next::read()
    }

    fn on_request_writable(&mut self, _encoder: &mut Encoder<HttpStream>) -> Next {
        Next::read()
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
            Next::read()
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
                    Next::read()
                }
            },
            Err(e) => match e.kind() {
                io::ErrorKind::WouldBlock => Next::read(),
                _ => panic!("io read error: {:?}", e)
            }
        }
    }
}

struct Client {
    client: Option<hyper::Client<TestHandler>>,
}

impl Client {
    fn request<U>(&self, url: U, method: Method) -> mpsc::Receiver<Msg>
    where U: AsRef<str> {
        let (handler, rx) = TestHandler::new(method);
        self.client.as_ref().unwrap()
            .request(url.as_ref().parse().unwrap(), handler);
        rx
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        self.client.take().map(|c| c.close());
    }
}

fn client() -> Client {
    let c = hyper::Client::new().unwrap();
    Client {
        client: Some(c),
    }
}


#[test]
fn client_get() {
    extern crate env_logger;
    env_logger::init().unwrap();
    let server = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = server.local_addr().unwrap();
    let client = client();
    let res = client.request(format!("http://{}/", addr), Method::Get);

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
