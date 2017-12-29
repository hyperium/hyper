#![deny(warnings)]
extern crate futures;
extern crate hyper;
extern crate pretty_env_logger;

use futures::{Future, Sink};
use futures::sync::{mpsc, oneshot};

use hyper::{Chunk, Get, StatusCode};
use hyper::error::Error;
use hyper::header::ContentLength;
use hyper::server::{Http, Service, Request, Response};

use std::fs::File;
use std::io::{self, copy, Read};
use std::thread;

static NOTFOUND: &[u8] = b"Not Found";
static INDEX: &str = "examples/send_file_index.html";

fn simple_file_send(f: &str) -> Box<Future<Item = Response, Error = hyper::Error>> {
    // Serve a file by reading it entirely into memory. As a result
    // this is limited to serving small files, but it is somewhat
    // simpler with a little less overhead.
    //
    // On channel errors, we panic with the expect method. The thread
    // ends at that point in any case.
    let filename = f.to_string(); // we need to copy for lifetime issues
    let (tx, rx) = oneshot::channel();
    thread::spawn(move || {
        let mut file = match File::open(filename) {
            Ok(f) => f,
            Err(_) => {
                tx.send(Response::new()
                        .with_status(StatusCode::NotFound)
                        .with_header(ContentLength(NOTFOUND.len() as u64))
                        .with_body(NOTFOUND))
                    .expect("Send error on open");
                return;
            },
        };
        let mut buf: Vec<u8> = Vec::new();
        match copy(&mut file, &mut buf) {
            Ok(_) => {
                let res = Response::new()
                    .with_header(ContentLength(buf.len() as u64))
                    .with_body(buf);
                tx.send(res).expect("Send error on successful file read");
            },
            Err(_) => {
                tx.send(Response::new().with_status(StatusCode::InternalServerError)).
                    expect("Send error on error reading file");
            },
        };
    });

    Box::new(rx.map_err(|e| Error::from(io::Error::new(io::ErrorKind::Other, e))))
}

struct ResponseExamples;

impl Service for ResponseExamples {
    type Request = Request;
    type Response = Response;
    type Error = hyper::Error;
    type Future = Box<Future<Item = Self::Response, Error = Self::Error>>;

    fn call(&self, req: Request) -> Self::Future {
        match (req.method(), req.path()) {
            (&Get, "/") | (&Get, "/index.html") => {
                simple_file_send(INDEX)
            },
            (&Get, "/big_file.html") => {
                // Stream a large file in chunks. This requires a
                // little more overhead with two channels, (one for
                // the response future, and a second for the response
                // body), but can handle arbitrarily large files.
                //
                // We use an artificially small buffer, since we have
                // a small test file.
                let (tx, rx) = oneshot::channel();
                thread::spawn(move || {
                    let mut file = match File::open(INDEX) {
                        Ok(f) => f,
                        Err(_) => {
                            tx.send(Response::new()
                                    .with_status(StatusCode::NotFound)
                                    .with_header(ContentLength(NOTFOUND.len() as u64))
                                    .with_body(NOTFOUND))
                                .expect("Send error on open");
                            return;
                        },
                    };
                    let (mut tx_body, rx_body) = mpsc::channel(1);
                    let res = Response::new().with_body(rx_body);
                    tx.send(res).expect("Send error on successful file read");
                    let mut buf = [0u8; 16];
                    loop {
                        match file.read(&mut buf) {
                            Ok(n) => {
                                if n == 0 {
                                    // eof
                                    tx_body.close().expect("panic closing");
                                    break;
                                } else {
                                    let chunk: Chunk = buf.to_vec().into();
                                    match tx_body.send(Ok(chunk)).wait() {
                                        Ok(t) => { tx_body = t; },
                                        Err(_) => { break; }
                                    };
                                }
                            },
                            Err(_) => { break; }
                        }
                    }
                });
                
                Box::new(rx.map_err(|e| Error::from(io::Error::new(io::ErrorKind::Other, e))))
            },
            (&Get, "/no_file.html") => {
                // Test what happens when file cannot be be found
                simple_file_send("this_file_should_not_exist.html")
            },
            _ => {
                Box::new(futures::future::ok(Response::new()
                                    .with_status(StatusCode::NotFound)))
            }
        }
    }

}


fn main() {
    pretty_env_logger::init().unwrap();
    let addr = "127.0.0.1:1337".parse().unwrap();

    let server = Http::new().bind(&addr, || Ok(ResponseExamples)).unwrap();
    println!("Listening on http://{} with 1 thread.", server.local_addr().unwrap());
    server.run().unwrap();
}
