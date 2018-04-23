#![deny(warnings)]
extern crate hyper;
extern crate pretty_env_logger;

use hyper::{Body, Method, Request, Response, Server, StatusCode};
use hyper::service::service_fn_ok;
use hyper::rt::Future;

static INDEX: &'static [u8] = b"Try POST /echo";

// Using service_fn_ok, we can convert this function into a `Service`.
fn echo(req: Request<Body>) -> Response<Body> {
   match (req.method(), req.uri().path()) {
        (&Method::GET, "/") | (&Method::POST, "/") => {
            Response::new(INDEX.into())
        },
        (&Method::POST, "/echo") => {
            Response::new(req.into_body())
        },
        _ => {
            let mut res = Response::new(Body::empty());
            *res.status_mut() = StatusCode::NOT_FOUND;
            res
        }
    }
}


fn main() {
    pretty_env_logger::init();

    let addr = ([127, 0, 0, 1], 1337).into();

    let server = Server::bind(&addr)
        .serve(|| service_fn_ok(echo))
        .map_err(|e| eprintln!("server error: {}", e));

    println!("Listening on http://{}", addr);

    hyper::rt::run(server);
}
