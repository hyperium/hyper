#![deny(warnings)]
extern crate futures;
extern crate hyper;
extern crate pretty_env_logger;
extern crate url;

use futures::{Future, Stream};

use hyper::{Get, Post, StatusCode};
use hyper::header::ContentLength;
use hyper::server::{Http, Service, Request, Response};

use std::collections::HashMap;
use url::form_urlencoded;

static INDEX: &[u8] = b"<html><body><form action=\"post\" method=\"post\">Name: <input type=\"text\" name=\"name\"><br>Number: <input type=\"text\" name=\"number\"><br><input type=\"submit\"></body></html>";
static MISSING: &[u8] = b"Missing field";
static NOTNUMERIC: &[u8] = b"Number field is not numeric";

struct ParamExample;

impl Service for ParamExample {
    type Request = Request;
    type Response = Response;
    type Error = hyper::Error;
    type Future = Box<Future<Item = Self::Response, Error = Self::Error>>;

    fn call(&self, req: Request) -> Self::Future {
        match (req.method(), req.path()) {
            (&Get, "/") | (&Get, "/post") => {
                Box::new(futures::future::ok(Response::new()
                                             .with_header(ContentLength(INDEX.len() as u64))
                                             .with_body(INDEX)))
            },
            (&Post, "/post") => {
                Box::new(req.body().concat2().map(|b| {
                    // Parse the request body. form_urlencoded::parse
                    // always succeeds, but in general parsing may
                    // fail (for example, an invalid post of json), so
                    // returning early with BadRequest may be
                    // necessary.
                    //
                    // Warning: this is a simplified use case. In
                    // principle names can appear multiple times in a
                    // form, and the values should be rolled up into a
                    // HashMap<String, Vec<String>>. However in this
                    // example the simpler approach is sufficient.
                    let params = form_urlencoded::parse(b.as_ref()).into_owned().collect::<HashMap<String, String>>();

                    // Validate the request parameters, returning
                    // early if an invalid input is detected.
                    let name = if let Some(n) = params.get("name") {
                        n
                    } else {
                        return Response::new()
                            .with_status(StatusCode::UnprocessableEntity)
                            .with_header(ContentLength(MISSING.len() as u64))
                            .with_body(MISSING);
                    };
                    let number = if let Some(n) = params.get("number") {
                        if let Ok(v) = n.parse::<f64>() {
                            v
                        } else {
                            return Response::new()
                                .with_status(StatusCode::UnprocessableEntity)
                                .with_header(ContentLength(NOTNUMERIC.len() as u64))
                                .with_body(NOTNUMERIC);
                        }
                    } else {
                        return Response::new()
                            .with_status(StatusCode::UnprocessableEntity)
                            .with_header(ContentLength(MISSING.len() as u64))
                            .with_body(MISSING);
                    };

                    // Render the response. This will often involve
                    // calls to a database or web service, which will
                    // require creating a new stream for the response
                    // body. Since those may fail, other error
                    // responses such as InternalServiceError may be
                    // needed here, too.
                    let body = format!("Hello {}, your number is {}", name, number);
                    Response::new()
                        .with_header(ContentLength(body.len() as u64))
                        .with_body(body)
                }))
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

    let server = Http::new().bind(&addr, || Ok(ParamExample)).unwrap();
    println!("Listening on http://{} with 1 thread.", server.local_addr().unwrap());
    server.run().unwrap();
}
