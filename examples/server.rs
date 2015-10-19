#![deny(warnings)]
extern crate hyper;
extern crate env_logger;

use std::io::{copy};
use std::env;

use hyper::{Get, Post};
use hyper::error::Result;
use hyper::server::{Server, Request, Response, Listening, Handler};
use hyper::uri::RequestUri::AbsolutePath;

#[cfg(feature = "openssl")]
use hyper::net::Openssl;

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
                try_return!(res.send(b"Try POST /echo"));
                return;
            },
            (&Post, "/echo") => (), // fall through, fighting mutable borrows
            _ => {
                *res.status_mut() = hyper::NotFound;
                return;
            }
        },
        _ => {
            return;
        }
    };

    let mut res = try_return!(res.start());
    try_return!(copy(&mut req, &mut res));
}

#[derive(Debug)]
struct Config {
    crt: String,
    key: String
}

#[cfg(feature = "openssl")]
fn start_server<H: Handler + 'static>(config: & Option<Config>, handler: H) -> Result<(Listening,String)> {
    let url = "127.0.0.1:1337";
    match *config {
        Some(Config { ref crt, ref key}) => {
            let ssl = try!(Openssl::with_cert_and_key(crt, key));
            let handler = try!(Server::https(url, ssl).and_then(|s| s.handle(handler)));
            Ok((handler, String::from("https://") + url))
        },
        _ => {
                let handler = try!(Server::http(url).and_then(|s| s.handle(handler)));
                Ok((handler, String::from("http://") + url))
            }
    }
}

#[cfg(not(feature = "openssl"))]
fn start_server<H: Handler + 'static>(_: & Option<Config>, handler: H) -> Result<(Listening,String)> {
    let url = "127.0.0.1:1337";
    let handler = try!(Server::http(url).and_then(|s| s.handle(handler)));
    Ok((handler, String::from("http://") + url))
}

fn main() {
    env_logger::init().unwrap();
    let args: Vec<String> = env::args().collect();

    let config = if args.len() >= 3 && &*args[1] == "--ssl" {
        // syntax: --ssl folder-with-crt-and-key
        let dir = &args[2];
        Some(
            Config {
                crt: dir.clone() + "/server.crt",
                key: dir.clone() + "/server.key"
            }
        )
    } else {
        None
    };
    let (_guard, message) = start_server(&config, echo).unwrap();

    println!("Listening on: {}", message);
}
