#![deny(warnings)]
extern crate hyper;
extern crate env_logger;
#[macro_use]
extern crate log;

use std::io::{self, Read, Write};

use hyper::{Get, Post, StatusCode, RequestUri, Decoder, Encoder, Next};
use hyper::header::ContentLength;
use hyper::net::HttpStream;
use hyper::server::{Server, Handler, Request, Response};

struct Echo {
    buf: Vec<u8>,
    read_pos: usize,
    write_pos: usize,
    eof: bool,
    route: Route,
}

enum Route {
    NotFound,
    Index,
    Echo(Body),
}

#[derive(Clone, Copy)]
enum Body {
    Len(u64),
    Chunked
}

static INDEX: &'static [u8] = b"Try POST /echo";

impl Echo {
    fn new() -> Echo {
        Echo {
            buf: vec![0; 4096],
            read_pos: 0,
            write_pos: 0,
            eof: false,
            route: Route::NotFound,
        }
    }
}

impl Handler<HttpStream> for Echo {
    fn on_request(&mut self, req: Request<HttpStream>) -> Next {
        match *req.uri() {
            RequestUri::AbsolutePath(ref path) => match (req.method(), &path[..]) {
                (&Get, "/") | (&Get, "/echo") => {
                    info!("GET Index");
                    self.route = Route::Index;
                    Next::write()
                }
                (&Post, "/echo") => {
                    info!("POST Echo");
                    let mut is_more = true;
                    self.route = if let Some(len) = req.headers().get::<ContentLength>() {
                        is_more = **len > 0;
                        Route::Echo(Body::Len(**len))
                    } else {
                        Route::Echo(Body::Chunked)
                    };
                    if is_more {
                        Next::read_and_write()
                    } else {
                        Next::write()
                    }
                }
                _ => Next::write(),
            },
            _ => Next::write()
        }
    }
    fn on_request_readable(&mut self, transport: &mut Decoder<HttpStream>) -> Next {
        match self.route {
            Route::Echo(ref body) => {
                if self.read_pos < self.buf.len() {
                    match transport.read(&mut self.buf[self.read_pos..]) {
                        Ok(0) => {
                            debug!("Read 0, eof");
                            self.eof = true;
                            Next::write()
                        },
                        Ok(n) => {
                            self.read_pos += n;
                            match *body {
                                Body::Len(max) if max <= self.read_pos as u64 => {
                                    self.eof = true;
                                    Next::write()
                                },
                                _ => Next::read_and_write()
                            }
                        }
                        Err(e) => match e.kind() {
                            io::ErrorKind::WouldBlock => Next::read_and_write(),
                            _ => {
                                println!("read error {:?}", e);
                                Next::end()
                            }
                        }
                    }
                } else {
                    Next::write()
                }
            }
            _ => unreachable!()
        }
    }

    fn on_response(&mut self, res: &mut Response) -> Next {
        match self.route {
            Route::NotFound => {
                res.set_status(StatusCode::NotFound);
                Next::end()
            }
            Route::Index => {
                res.headers_mut().set(ContentLength(INDEX.len() as u64));
                Next::write()
            }
            Route::Echo(body) => {
                if let Body::Len(len) = body {
                    res.headers_mut().set(ContentLength(len));
                }
                Next::read_and_write()
            }
        }
    }

    fn on_response_writable(&mut self, transport: &mut Encoder<HttpStream>) -> Next {
        match self.route {
            Route::Index => {
                transport.write(INDEX).unwrap();
                Next::end()
            }
            Route::Echo(..) => {
                if self.write_pos < self.read_pos {
                    match transport.write(&self.buf[self.write_pos..self.read_pos]) {
                        Ok(0) => panic!("write ZERO"),
                        Ok(n) => {
                            self.write_pos += n;
                            Next::write()
                        }
                        Err(e) => match e.kind() {
                            io::ErrorKind::WouldBlock => Next::write(),
                            _ => {
                                println!("write error {:?}", e);
                                Next::end()
                            }
                        }
                    }
                } else if !self.eof {
                    Next::read()
                } else {
                    Next::end()
                }
            }
            _ => unreachable!()
        }
    }
}

fn main() {
    env_logger::init().unwrap();
    let server = Server::http(&"127.0.0.1:1337".parse().unwrap()).unwrap();
    let (listening, server) = server.handle(|_| Echo::new()).unwrap();
    println!("Listening on http://{}", listening);
    server.run();
}
