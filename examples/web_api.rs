#![deny(warnings)]
extern crate futures;
extern crate hyper;
extern crate pretty_env_logger;
extern crate serde_json;

use futures::{future, Future, Stream};

use hyper::{Body, Chunk, Client, Method, Request, Response, Server, StatusCode, header};
use hyper::client::HttpConnector;
use hyper::service::service_fn;

#[allow(unused, deprecated)]
use std::ascii::AsciiExt;

static NOTFOUND: &[u8] = b"Not Found";
static URL: &str = "http://127.0.0.1:1337/web_api";
static INDEX: &[u8] = b"<a href=\"test.html\">test.html</a>";
static LOWERCASE: &[u8] = b"i am a lower case string";

fn response_examples(req: Request<Body>, client: &Client<HttpConnector>)
    -> Box<Future<Item=Response<Body>, Error=hyper::Error> + Send>
{
    match (req.method(), req.uri().path()) {
        (&Method::GET, "/") | (&Method::GET, "/index.html") => {
            let body = Body::from(INDEX);
            Box::new(future::ok(Response::new(body)))
        },
        (&Method::GET, "/test.html") => {
            // Run a web query against the web api below

            // build the request
            let req = Request::builder()
                .method(Method::POST)
                .uri(URL)
                .body(LOWERCASE.into())
                .unwrap();
            // use the request with client
            let web_res_future = client.request(req);

            Box::new(web_res_future.map(|web_res| {
                // return the response that came from the web api and the original text together
                // to show the difference
                let body = Body::wrap_stream(web_res.into_body().map(|b| {
                    Chunk::from(format!("<b>before</b>: {}<br><b>after</b>: {}",
                                        std::str::from_utf8(LOWERCASE).unwrap(),
                                        std::str::from_utf8(&b).unwrap()))
                }));

                Response::new(body)
            }))
        },
        (&Method::POST, "/web_api") => {
            // A web api to run against. Uppercases the body and returns it back.
            let body = Body::wrap_stream(req.into_body().map(|chunk| {
                // uppercase the letters
                let upper = chunk.iter().map(|byte| byte.to_ascii_uppercase())
                    .collect::<Vec<u8>>();
                Chunk::from(upper)
            }));
            Box::new(future::ok(Response::new(body)))
        },
        (&Method::GET, "/json") => {
            let data = vec!["foo", "bar"];
            let res = match serde_json::to_string(&data) {
                Ok(json) => {
                    // return a json response
                    Response::builder()
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(Body::from(json))
                        .unwrap()
                }
                // This is unnecessary here because we know
                // this can't fail. But if we were serializing json that came from another
                // source we could handle an error like this.
                Err(e) => {
                    eprintln!("serializing json: {}", e);

                    Response::builder()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .body(Body::from("Internal Server Error"))
                        .unwrap()
                }
            };

            Box::new(future::ok(res))
        }
        _ => {
            // Return 404 not found response.
            let body = Body::from(NOTFOUND);
            Box::new(future::ok(Response::builder()
                                         .status(StatusCode::NOT_FOUND)
                                         .body(body)
                                         .unwrap()))
        }
    }
}

fn main() {
    pretty_env_logger::init();

    let addr = "127.0.0.1:1337".parse().unwrap();

    hyper::rt::run(future::lazy(move || {
        // Share a `Client` with all `Service`s
        let client = Client::new();

        let new_service = move || {
            // Move a clone of `client` into the `service_fn`.
            let client = client.clone();
            service_fn(move |req| {
                response_examples(req, &client)
            })
        };

        let server = Server::bind(&addr)
            .serve(new_service)
            .map_err(|e| eprintln!("server error: {}", e));

        println!("Listening on http://{}", addr);

        server
    }));
}

