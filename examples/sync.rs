extern crate hyper;
extern crate env_logger;
extern crate time;

use std::io::{self, Read, Write};
use std::marker::PhantomData;
use std::thread;
use std::sync::{Arc, mpsc};

pub struct Server {
    listening: hyper::server::Listening,
}

pub struct Request<'a> {
    #[allow(dead_code)]
    inner: hyper::server::Request,
    tx: &'a mpsc::Sender<Action>,
    rx: &'a mpsc::Receiver<io::Result<usize>>,
    ctrl: &'a hyper::Control,
}

impl<'a> Request<'a> {
    fn new(inner: hyper::server::Request, tx: &'a mpsc::Sender<Action>, rx: &'a mpsc::Receiver<io::Result<usize>>, ctrl: &'a hyper::Control) -> Request<'a> {
        Request {
            inner: inner,
            tx: tx,
            rx: rx,
            ctrl: ctrl,
        }
    }
}

impl<'a> io::Read for Request<'a> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.tx.send(Action::Read(buf.as_mut_ptr(), buf.len())).unwrap();
        self.ctrl.ready(hyper::Next::read()).unwrap();
        self.rx.recv().unwrap()
    }
}

pub enum Fresh {}
pub enum Streaming {}

pub struct Response<'a, W = Fresh> {
    status: hyper::StatusCode,
    headers: hyper::Headers,
    version: hyper::HttpVersion,
    tx: &'a mpsc::Sender<Action>,
    rx: &'a mpsc::Receiver<io::Result<usize>>,
    ctrl: &'a hyper::Control,
    _marker: PhantomData<W>,
}

impl<'a> Response<'a, Fresh> {
    fn new(tx: &'a mpsc::Sender<Action>, rx: &'a mpsc::Receiver<io::Result<usize>>, ctrl: &'a hyper::Control) -> Response<'a, Fresh> {
        Response {
            status: hyper::Ok,
            headers: hyper::Headers::new(),
            version: hyper::HttpVersion::Http11,
            tx: tx,
            rx: rx,
            ctrl: ctrl,
            _marker: PhantomData,
        }
    }

    pub fn start(self) -> io::Result<Response<'a, Streaming>> {
        self.tx.send(Action::Respond(self.version.clone(), self.status.clone(), self.headers.clone())).unwrap();
        self.ctrl.ready(hyper::Next::write()).unwrap();
        let res = self.rx.recv().unwrap();
        res.map(move |_| Response {
            status: self.status,
            headers: self.headers,
            version: self.version,
            tx: self.tx,
            rx: self.rx,
            ctrl: self.ctrl,
            _marker: PhantomData,
        })
    }

    pub fn send(mut self, msg: &[u8]) -> io::Result<()> {
        self.headers.set(hyper::header::ContentLength(msg.len() as u64));
        self.start().and_then(|mut res| res.write_all(msg)).map(|_| ())
    }
}

impl<'a> Write for Response<'a, Streaming> {
    fn write(&mut self, msg: &[u8]) -> io::Result<usize> {
        self.tx.send(Action::Write(msg.as_ptr(), msg.len())).unwrap();
        self.ctrl.ready(hyper::Next::write()).unwrap();
        let res = self.rx.recv().unwrap();
        res
    }

    fn flush(&mut self) -> io::Result<()> {
        panic!("Response.flush() not impemented")
    }
}

struct SynchronousHandler {
    req_tx: mpsc::Sender<hyper::server::Request>,
    tx: mpsc::Sender<io::Result<usize>>,
    rx: mpsc::Receiver<Action>,
    reading: Option<(*mut u8, usize)>,
    writing: Option<(*const u8, usize)>,
    respond: Option<(hyper::HttpVersion, hyper::StatusCode, hyper::Headers)>
}

unsafe impl Send for SynchronousHandler {}

impl SynchronousHandler {
    fn next(&mut self) -> hyper::Next {
        match self.rx.try_recv() {
            Ok(Action::Read(ptr, len)) => {
                self.reading = Some((ptr, len));
                hyper::Next::read()
            },
            Ok(Action::Respond(ver, status, headers)) => {
                self.respond = Some((ver, status, headers));
                hyper::Next::write()
            },
            Ok(Action::Write(ptr, len)) => {
                self.writing = Some((ptr, len));
                hyper::Next::write()
            }
            Err(mpsc::TryRecvError::Empty) => {
                // we're too fast, the other thread hasn't had a chance to respond
                hyper::Next::wait()
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                // they dropped it
                // TODO: should finish up sending response, whatever it was
                hyper::Next::end()
            }
        }
    }

    fn reading(&mut self) -> Option<(*mut u8, usize)> {
        self.reading.take().or_else(|| {
            match self.rx.try_recv() {
                Ok(Action::Read(ptr, len)) => {
                    Some((ptr, len))
                },
                _ => None
            }
        })
    }

    fn writing(&mut self) -> Option<(*const u8, usize)> {
        self.writing.take().or_else(|| {
            match self.rx.try_recv() {
                Ok(Action::Write(ptr, len)) => {
                    Some((ptr, len))
                },
                _ => None
            }
        })
    }
    fn respond(&mut self) -> Option<(hyper::HttpVersion, hyper::StatusCode, hyper::Headers)> {
        self.respond.take().or_else(|| {
            match self.rx.try_recv() {
                Ok(Action::Respond(ver, status, headers)) => {
                    Some((ver, status, headers))
                },
                _ => None
            }
        })
    }
}

impl hyper::server::Handler<hyper::net::HttpStream> for SynchronousHandler {
    fn on_request(&mut self, req: hyper::server::Request) -> hyper::Next {
        if let Err(_) = self.req_tx.send(req) {
            return hyper::Next::end();
        }

        self.next()
    }

    fn on_request_readable(&mut self, decoder: &mut hyper::Decoder<hyper::net::HttpStream>) -> hyper::Next {
        if let Some(raw) = self.reading() {
            let slice = unsafe { ::std::slice::from_raw_parts_mut(raw.0, raw.1) };
            if self.tx.send(decoder.read(slice)).is_err() {
                return hyper::Next::end();
            }
        }
        self.next()
    }

    fn on_response(&mut self, req: &mut hyper::server::Response) -> hyper::Next {
        use std::iter::Extend;
        if let Some(head) = self.respond() {
            req.set_status(head.1);
            req.headers_mut().extend(head.2.iter());
            if self.tx.send(Ok(0)).is_err() {
                return hyper::Next::end();
            }
        } else {
            // wtf happened?
            panic!("no head to respond with");
        }
        self.next()
    }

    fn on_response_writable(&mut self, encoder: &mut hyper::Encoder<hyper::net::HttpStream>) -> hyper::Next {
        if let Some(raw) = self.writing() {
            let slice = unsafe { ::std::slice::from_raw_parts(raw.0, raw.1) };
            if self.tx.send(encoder.write(slice)).is_err() {
                return hyper::Next::end();
            }
        }
        self.next()
    }
}

enum Action {
    Read(*mut u8, usize),
    Write(*const u8, usize),
    Respond(hyper::HttpVersion, hyper::StatusCode, hyper::Headers),
}

unsafe impl Send for Action {}

trait Handler: Send + Sync + 'static {
    fn handle(&self, req: Request, res: Response);
}

impl<F> Handler for F where F: Fn(Request, Response) + Send + Sync + 'static {
    fn handle(&self, req: Request, res: Response) {
        (self)(req, res)
    }
}

impl Server {
    fn handle<H: Handler>(addr: &str, handler: H) -> Server {
        let handler = Arc::new(handler);
        let (listening, server) = hyper::Server::http(&addr.parse().unwrap()).unwrap()
            .handle(move |ctrl| {
                let (req_tx, req_rx) = mpsc::channel();
                let (blocking_tx, blocking_rx) = mpsc::channel();
                let (async_tx, async_rx) = mpsc::channel();
                let handler = handler.clone();
                thread::Builder::new().name("handler-thread".into()).spawn(move || {
                    let req = Request::new(req_rx.recv().unwrap(), &blocking_tx, &async_rx, &ctrl);
                    let res = Response::new(&blocking_tx, &async_rx, &ctrl);
                    handler.handle(req, res);
                }).unwrap();

                SynchronousHandler {
                    req_tx: req_tx,
                    tx: async_tx,
                    rx: blocking_rx,
                    reading: None,
                    writing: None,
                    respond: None,
                }
            }).unwrap();
        thread::spawn(move || {
            server.run();
        });
        Server {
            listening: listening
        }
    }
}

fn main() {
    env_logger::init().unwrap();
    let s = Server::handle("127.0.0.1:0", |mut req: Request, res: Response| {
        let mut body = [0; 256];
        let n = req.read(&mut body).unwrap();
        println!("!!!: received: {:?}", ::std::str::from_utf8(&body[..n]).unwrap());

        res.send(b"Hello World!").unwrap();
    });
    println!("listening on {}", s.listening.addr());
}
