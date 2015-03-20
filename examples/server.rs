#![deny(warnings)]
extern crate hyper;
extern crate env_logger;

use std::io::{Write, copy};
use std::net::Ipv4Addr;

use hyper::{Get, Post};
use hyper::header::ContentLength;
use hyper::server::{Server, Request, Response};
use hyper::uri::RequestUri::AbsolutePath;

macro_rules! try_return(
    ($e:expr) => {{
        match $e {
            Ok(v) => v,
            Err(e) => { println!("Error: {}", e); return; }
        }
    }}
);

fn echo(mut req: Request, mut res: Response) {
    match req.uri {
        AbsolutePath(ref path) => match (&req.method, &path[..]) {
            (&Get, "/") | (&Get, "/echo") => {
                let out = b"Try POST /echo";

                res.headers_mut().set(ContentLength(out.len() as u64));
                let mut res = try_return!(res.start());
                try_return!(res.write_all(out));
                try_return!(res.end());
                return;
            },
            (&Post, "/echo") => (), // fall through, fighting mutable borrows
            _ => {
                *res.status_mut() = hyper::NotFound;
                try_return!(res.start().and_then(|res| res.end()));
                return;
            }
        },
        _ => {
            try_return!(res.start().and_then(|res| res.end()));
            return;
        }
    };

    let mut res = try_return!(res.start());
    try_return!(copy(&mut req, &mut res));
    try_return!(res.end());
}

fn main() {
    env_logger::init().unwrap();
    let server = Server::http(echo);
    let _guard = server.listen(Ipv4Addr::new(127, 0, 0, 1), 1337).unwrap();
    println!("Listening on http://127.0.0.1:1337");
}
