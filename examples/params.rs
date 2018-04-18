#![deny(warnings)]
extern crate futures;
extern crate hyper;
extern crate pretty_env_logger;
extern crate tokio;
extern crate url;

use futures::{future, Future, Stream};

use hyper::{Body, Method, Request, Response, Server, StatusCode};
use hyper::service::service_fn;

use std::collections::HashMap;
use url::form_urlencoded;

static INDEX: &[u8] = b"<html><body><form action=\"post\" method=\"post\">Name: <input type=\"text\" name=\"name\"><br>Number: <input type=\"text\" name=\"number\"><br><input type=\"submit\"></body></html>";
static MISSING: &[u8] = b"Missing field";
static NOTNUMERIC: &[u8] = b"Number field is not numeric";

// Using service_fn, we can turn this function into a `Service`.
fn param_example(req: Request<Body>) -> Box<Future<Item=Response<Body>, Error=hyper::Error> + Send> {
    match (req.method(), req.uri().path()) {
        (&Method::GET, "/") | (&Method::GET, "/post") => {
            Box::new(future::ok(Response::new(INDEX.into())))
        },
        (&Method::POST, "/post") => {
            Box::new(req.into_body().concat2().map(|b| {
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
                    return Response::builder()
                        .status(StatusCode::UNPROCESSABLE_ENTITY)
                        .body(MISSING.into())
                        .unwrap();
                };
                let number = if let Some(n) = params.get("number") {
                    if let Ok(v) = n.parse::<f64>() {
                        v
                    } else {
                        return Response::builder()
                            .status(StatusCode::UNPROCESSABLE_ENTITY)
                            .body(NOTNUMERIC.into())
                            .unwrap();
                    }
                } else {
                    return Response::builder()
                        .status(StatusCode::UNPROCESSABLE_ENTITY)
                        .body(MISSING.into())
                        .unwrap();
                };

                // Render the response. This will often involve
                // calls to a database or web service, which will
                // require creating a new stream for the response
                // body. Since those may fail, other error
                // responses such as InternalServiceError may be
                // needed here, too.
                let body = format!("Hello {}, your number is {}", name, number);
                Response::new(body.into())
            }))
        },
        _ => {
            Box::new(future::ok(Response::builder()
                                .status(StatusCode::NOT_FOUND)
                                .body(Body::empty())
                                .unwrap()))
        }
    }

}

fn main() {
    pretty_env_logger::init();

    let addr = ([127, 0, 0, 1], 1337).into();

    let server = Server::bind(&addr)
        .serve(|| service_fn(param_example))
        .map_err(|e| eprintln!("server error: {}", e));

    tokio::run(server);
}
