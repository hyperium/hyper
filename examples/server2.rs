// Basically the same as server.rs, but crafting a custom event loop and
// listener, running the HTTP protocol on top of it.

#![deny(warnings)]
extern crate futures;
extern crate hyper;
extern crate pretty_env_logger;
extern crate tokio_core;

use futures::future::FutureResult;
use futures::Stream;

use tokio_core::reactor::Core;
use tokio_core::net::TcpListener;

use hyper::{Get, Post, StatusCode};
use hyper::header::ContentLength;
use hyper::server::{Http, Service, Request, Response};

static INDEX: &'static [u8] = b"Try POST /echo";

#[derive(Clone, Copy)]
struct Echo;

impl Service for Echo {
    type Request = Request;
    type Response = Response;
    type Error = hyper::Error;
    type Future = FutureResult<Response, hyper::Error>;

    fn call(&self, req: Request) -> Self::Future {
        futures::future::ok(match (req.method(), req.path()) {
            (&Get, "/") | (&Get, "/echo") => {
                Response::new()
                    .with_header(ContentLength(INDEX.len() as u64))
                    .with_body(INDEX)
            },
            (&Post, "/echo") => {
                let mut res = Response::new();
                if let Some(len) = req.headers().get::<ContentLength>() {
                    res.headers_mut().set(len.clone());
                }
                res.with_body(req.body())
            },
            _ => {
                Response::new()
                    .with_status(StatusCode::NotFound)
            }
        })
    }
}

fn main() {
    pretty_env_logger::init().expect("unable to initialize the env logger");
    let addr = "[::1]:1337".parse().unwrap();
    let http = Http::new();

    let mut lp = Core::new().expect("unable to initialize the main event loop");
    let handle = lp.handle();
    let listener = TcpListener::bind(&addr, &lp.handle()).expect("unable to listen");
    println!("Listening on http://{} with 1 thread.", listener.local_addr().unwrap());

    let service_factory = || Echo {};
    let srv = listener.incoming().for_each(move |(socket, addr)| {
        http.bind_connection(&handle, socket, addr, service_factory());
        Ok(())
    });

    lp.run(srv).expect("error running the event loop");
}
