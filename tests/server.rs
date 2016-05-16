#![deny(warnings)]
extern crate hyper;

use std::net::{TcpStream, SocketAddr};
use std::io::{self, Read, Write};
use std::sync::mpsc;
use std::time::Duration;

use hyper::{Next, Encoder, Decoder};
use hyper::net::HttpStream;
use hyper::server::{Server, Handler, Request, Response};

struct Serve {
    listening: Option<hyper::server::Listening>,
    msg_rx: mpsc::Receiver<Msg>,
    reply_tx: mpsc::Sender<Reply>,
}

impl Serve {
    fn addr(&self) -> &SocketAddr {
        self.listening.as_ref().unwrap().addr()
    }

    /*
    fn head(&self) -> Request {
        unimplemented!()
    }
    */

    fn body(&self) -> Vec<u8> {
        let mut buf = vec![];
        while let Ok(Msg::Chunk(msg)) = self.msg_rx.try_recv() {
            buf.extend(&msg);
        }
        buf
    }

    fn reply(&self) -> ReplyBuilder {
        ReplyBuilder {
            tx: &self.reply_tx
        }
    }
}

struct ReplyBuilder<'a> {
    tx: &'a mpsc::Sender<Reply>,
}

impl<'a> ReplyBuilder<'a> {
    fn status(self, status: hyper::StatusCode) -> Self {
        self.tx.send(Reply::Status(status)).unwrap();
        self
    }

    fn header<H: hyper::header::Header>(self, header: H) -> Self {
        let mut headers = hyper::Headers::new();
        headers.set(header);
        self.tx.send(Reply::Headers(headers)).unwrap();
        self
    }

    fn body<T: AsRef<[u8]>>(self, body: T) {
        self.tx.send(Reply::Body(body.as_ref().into())).unwrap();
    }
}

impl Drop for Serve {
    fn drop(&mut self) {
        self.listening.take().unwrap().close();
    }
}

struct TestHandler {
    tx: mpsc::Sender<Msg>,
    rx: mpsc::Receiver<Reply>,
    peeked: Option<Vec<u8>>,
    timeout: Option<Duration>,
}

enum Reply {
    Status(hyper::StatusCode),
    Headers(hyper::Headers),
    Body(Vec<u8>),
}

enum Msg {
    //Head(Request),
    Chunk(Vec<u8>),
}

impl TestHandler {
    fn next(&self, next: Next) -> Next {
        if let Some(dur) = self.timeout {
            next.timeout(dur)
        } else {
            next
        }
    }
}

impl Handler<HttpStream> for TestHandler {
    fn on_request(&mut self, _req: Request) -> Next {
        //self.tx.send(Msg::Head(req)).unwrap();
        self.next(Next::read())
    }

    fn on_request_readable(&mut self, decoder: &mut Decoder<HttpStream>) -> Next {
        let mut vec = vec![0; 1024];
        match decoder.read(&mut vec) {
            Ok(0) => {
                self.next(Next::write())
            }
            Ok(n) => {
                vec.truncate(n);
                self.tx.send(Msg::Chunk(vec)).unwrap();
                self.next(Next::read())
            }
            Err(e) => match e.kind() {
                io::ErrorKind::WouldBlock => self.next(Next::read()),
                _ => panic!("test error: {}", e)
            }
        }
    }

    fn on_response(&mut self, res: &mut Response) -> Next {
        loop {
            match self.rx.try_recv() {
                Ok(Reply::Status(s)) => {
                    res.set_status(s);
                },
                Ok(Reply::Headers(headers)) => {
                    use std::iter::Extend;
                    res.headers_mut().extend(headers.iter());
                },
                Ok(Reply::Body(body)) => {
                    self.peeked = Some(body);
                },
                Err(..) => {
                    return if self.peeked.is_some() {
                        self.next(Next::write())
                    } else {
                        self.next(Next::end())
                    };
                },
            }
        }

    }

    fn on_response_writable(&mut self, encoder: &mut Encoder<HttpStream>) -> Next {
        match self.peeked {
            Some(ref body) => {
                encoder.write(body).unwrap();
                self.next(Next::end())
            },
            None => self.next(Next::end())
        }
    }
}

fn serve() -> Serve {
    serve_with_timeout(None)
}

fn serve_with_timeout(dur: Option<Duration>) -> Serve {
    use std::thread;

    let (msg_tx, msg_rx) = mpsc::channel();
    let (reply_tx, reply_rx) = mpsc::channel();
    let mut reply_rx = Some(reply_rx);
    let (listening, server) = Server::http(&"127.0.0.1:0".parse().unwrap()).unwrap()
        .handle(move |_| TestHandler {
            tx: msg_tx.clone(),
            timeout: dur,
            rx: reply_rx.take().unwrap(),
            peeked: None,
        }).unwrap();


    let thread_name = format!("test-server-{}: {:?}", listening.addr(), dur);
    thread::Builder::new().name(thread_name).spawn(move || {
        server.run();
    }).unwrap();

    Serve {
        listening: Some(listening),
        msg_rx: msg_rx,
        reply_tx: reply_tx,
    }
}

#[test]
fn server_get_should_ignore_body() {
    let server = serve();

    let mut req = TcpStream::connect(server.addr()).unwrap();
    req.write_all(b"\
        GET / HTTP/1.1\r\n\
        Host: example.domain\r\n\
        \r\n\
        I shouldn't be read.\r\n\
    ").unwrap();
    req.read(&mut [0; 256]).unwrap();

    assert_eq!(server.body(), b"");
}

#[test]
fn server_get_with_body() {
    let server = serve();
    let mut req = TcpStream::connect(server.addr()).unwrap();
    req.write_all(b"\
        GET / HTTP/1.1\r\n\
        Host: example.domain\r\n\
        Content-Length: 19\r\n\
        \r\n\
        I'm a good request.\r\n\
    ").unwrap();
    req.read(&mut [0; 256]).unwrap();

    // note: doesnt include trailing \r\n, cause Content-Length wasn't 21
    assert_eq!(server.body(), b"I'm a good request.");
}

#[test]
fn server_get_fixed_response() {
    let foo_bar = b"foo bar baz";
    let server = serve();
    server.reply()
        .status(hyper::Ok)
        .header(hyper::header::ContentLength(foo_bar.len() as u64))
        .body(foo_bar);
    let mut req = TcpStream::connect(server.addr()).unwrap();
    req.write_all(b"\
        GET / HTTP/1.1\r\n\
        Host: example.domain\r\n\
        Connection: close\r\n
        \r\n\
    ").unwrap();
    let mut body = String::new();
    req.read_to_string(&mut body).unwrap();
    let n = body.find("\r\n\r\n").unwrap() + 4;

    assert_eq!(&body[n..], "foo bar baz");
}

#[test]
fn server_get_chunked_response() {
    let foo_bar = b"foo bar baz";
    let server = serve();
    server.reply()
        .status(hyper::Ok)
        .header(hyper::header::TransferEncoding::chunked())
        .body(foo_bar);
    let mut req = TcpStream::connect(server.addr()).unwrap();
    req.write_all(b"\
        GET / HTTP/1.1\r\n\
        Host: example.domain\r\n\
        Connection: close\r\n
        \r\n\
    ").unwrap();
    let mut body = String::new();
    req.read_to_string(&mut body).unwrap();
    let n = body.find("\r\n\r\n").unwrap() + 4;

    assert_eq!(&body[n..], "B\r\nfoo bar baz\r\n0\r\n\r\n");
}

#[test]
fn server_post_with_chunked_body() {
    let server = serve();
    let mut req = TcpStream::connect(server.addr()).unwrap();
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
        \r\n
    ").unwrap();
    req.read(&mut [0; 256]).unwrap();

    assert_eq!(server.body(), b"qwert");
}

/*
#[test]
fn server_empty_response() {
    let server = serve();
    server.reply()
        .status(hyper::Ok);
    let mut req = TcpStream::connect(server.addr()).unwrap();
    req.write_all(b"\
        GET / HTTP/1.1\r\n\
        Host: example.domain\r\n\
        Connection: close\r\n
        \r\n\
    ").unwrap();

    let mut response = String::new();
    req.read_to_string(&mut response).unwrap();

    assert_eq!(response, "foo");
    assert!(!response.contains("Transfer-Encoding: chunked\r\n"));

    let mut lines = response.lines();
    assert_eq!(lines.next(), Some("HTTP/1.1 200 OK"));

    let mut lines = lines.skip_while(|line| !line.is_empty());
    assert_eq!(lines.next(), Some(""));
    assert_eq!(lines.next(), None);
}
*/

#[test]
fn server_empty_response_chunked() {
    let server = serve();
    server.reply()
        .status(hyper::Ok)
        .body("");
    let mut req = TcpStream::connect(server.addr()).unwrap();
    req.write_all(b"\
        GET / HTTP/1.1\r\n\
        Host: example.domain\r\n\
        Connection: close\r\n
        \r\n\
    ").unwrap();

    let mut response = String::new();
    req.read_to_string(&mut response).unwrap();

    assert!(response.contains("Transfer-Encoding: chunked\r\n"));

    let mut lines = response.lines();
    assert_eq!(lines.next(), Some("HTTP/1.1 200 OK"));

    let mut lines = lines.skip_while(|line| !line.is_empty());
    assert_eq!(lines.next(), Some(""));
    // 0\r\n\r\n
    assert_eq!(lines.next(), Some("0"));
    assert_eq!(lines.next(), Some(""));
    assert_eq!(lines.next(), None);
}

#[test]
fn server_empty_response_chunked_without_calling_write() {
    let server = serve();
    server.reply()
        .status(hyper::Ok)
        .header(hyper::header::TransferEncoding::chunked());
    let mut req = TcpStream::connect(server.addr()).unwrap();
    req.write_all(b"\
        GET / HTTP/1.1\r\n\
        Host: example.domain\r\n\
        Connection: close\r\n
        \r\n\
    ").unwrap();

    let mut response = String::new();
    req.read_to_string(&mut response).unwrap();

    assert!(response.contains("Transfer-Encoding: chunked\r\n"));

    let mut lines = response.lines();
    assert_eq!(lines.next(), Some("HTTP/1.1 200 OK"));

    let mut lines = lines.skip_while(|line| !line.is_empty());
    assert_eq!(lines.next(), Some(""));
    // 0\r\n\r\n
    assert_eq!(lines.next(), Some("0"));
    assert_eq!(lines.next(), Some(""));
    assert_eq!(lines.next(), None);
}
