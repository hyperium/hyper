extern crate hyper;
extern crate debug;

use std::io::{IoResult};
use std::io::util::copy;
use std::io::net::ip::Ipv4Addr;

use hyper::{Get, Post};
use hyper::server::{Server, Handler, Request, Response};
use hyper::header::ContentLength;

struct Echo;

impl Handler for Echo {
    fn handle(&mut self, mut req: Request, mut res: Response) -> IoResult<()> {
        match req.uri {
            hyper::uri::AbsolutePath(ref path) => match (&req.method, path.as_slice()) {
                (&Get, "/") | (&Get, "/echo") => {
                    let out = b"Try POST /echo";

                    res.headers.set(ContentLength(out.len()));
                    try!(res.write(out));
                    return res.end();
                },
                (&Post, "/echo") => (), // fall through, fighting mutable borrows
                _ => {
                    res.status = hyper::status::NotFound;
                    return res.end();
                }
            },
            _ => return res.end()
        };

        try!(copy(&mut req, &mut res));
        res.end()
    }
}

fn main() {
    let server = Server::http(Ipv4Addr(127, 0, 0, 1), 1337);
    server.listen(Echo).unwrap();
}
