//#![deny(warnings)]
extern crate futures;
extern crate hyper;
extern crate pretty_env_logger;
#[macro_use]
extern crate log;

use hyper::{Get, Post, StatusCode};
use hyper::header::ContentLength;
use hyper::server::{Server, Service, Request, Response};


static INDEX: &'static [u8] = b"Try POST /echo";

#[derive(Clone, Copy)]
struct Echo;

impl Service for Echo {
    type Request = Request;
    type Response = Response;
    type Error = hyper::Error;
    type Future = ::futures::Finished<Response, hyper::Error>;

    fn call(&self, req: Request) -> Self::Future {
        ::futures::finished(match (req.method(), req.path()) {
            (&Get, Some("/")) | (&Get, Some("/echo")) => {
                Response::new()
                    .header(ContentLength(INDEX.len() as u64))
                    .body(INDEX)
            },
            (&Post, Some("/echo")) => {
                let mut res = Response::new();
                if let Some(len) = req.headers().get::<ContentLength>() {
                    res = res.header(len.clone());
                }
                res.body(req.body())
            },
            _ => {
                Response::new()
                    .status(StatusCode::NotFound)
            }
        })
    }

    fn poll_ready(&self) -> ::futures::Async<()> {
        ::futures::Async::Ready(())
    }
}


fn main() {
    pretty_env_logger::init();
    let server = Server::http(&"127.0.0.1:1337".parse().unwrap()).unwrap();
    let (listening, server) = server.standalone(Echo).unwrap();
    println!("Listening on http://{}", listening);
    server.run();
}
