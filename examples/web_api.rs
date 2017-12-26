#![deny(warnings)]
extern crate futures;
extern crate hyper;
extern crate pretty_env_logger;
extern crate tokio_core;

use futures::{Future, Stream};

use hyper::{Body, Chunk, Client, Get, Post, StatusCode};
use hyper::error::Error;
use hyper::header::ContentLength;
use hyper::server::{Http, Service, Request, Response};

#[allow(unused)]
use std::ascii::AsciiExt;

static NOTFOUND: &[u8] = b"Not Found";
static URL: &str = "http://127.0.0.1:1337/web_api";
static INDEX: &[u8] = b"<a href=\"test.html\">test.html</a>";
static LOWERCASE: &[u8] = b"i am a lower case string";

pub type ResponseStream = Box<Stream<Item=Chunk, Error=Error>>;

struct ResponseExamples(tokio_core::reactor::Handle);

impl Service for ResponseExamples {
    type Request = Request;
    type Response = Response<ResponseStream>;
    type Error = hyper::Error;
    type Future = Box<Future<Item = Self::Response, Error = Self::Error>>;

    fn call(&self, req: Request) -> Self::Future {
        match (req.method(), req.path()) {
            (&Get, "/") | (&Get, "/index.html") => {
                let body: ResponseStream = Box::new(Body::from(INDEX));
                Box::new(futures::future::ok(Response::new()
                                             .with_header(ContentLength(INDEX.len() as u64))
                                             .with_body(body)))
            },
            (&Get, "/test.html") => {
                // Run a web query against the web api below
                let client = Client::configure().build(&self.0);
                let mut req = Request::new(Post, URL.parse().unwrap());
                req.set_body(LOWERCASE);
                let web_res_future = client.request(req);

                Box::new(web_res_future.map(|web_res| {
                    let body: ResponseStream = Box::new(web_res.body().map(|b| {
                        Chunk::from(format!("before: '{:?}'<br>after: '{:?}'",
                                            std::str::from_utf8(LOWERCASE).unwrap(),
                                            std::str::from_utf8(&b).unwrap()))
                    }));
                    Response::new().with_body(body)
                }))
            },
            (&Post, "/web_api") => {
                // A web api to run against. Simple upcasing of the body.
                let body: ResponseStream = Box::new(req.body().map(|chunk| {
                    let upper = chunk.iter().map(|byte| byte.to_ascii_uppercase())
                        .collect::<Vec<u8>>();
                    Chunk::from(upper)
                }));
                Box::new(futures::future::ok(Response::new().with_body(body)))
            },
            _ => {
                let body: ResponseStream = Box::new(Body::from(NOTFOUND));
                Box::new(futures::future::ok(Response::new()
                                             .with_status(StatusCode::NotFound)
                                             .with_header(ContentLength(NOTFOUND.len() as u64))
                                             .with_body(body)))
            }
        }
    }

}


fn main() {
    pretty_env_logger::init().unwrap();
    let addr = "127.0.0.1:1337".parse().unwrap();

    let mut core = tokio_core::reactor::Core::new().unwrap();
    let handle = core.handle();
    let client_handle = core.handle();

    let serve = Http::new().serve_addr_handle(&addr, &handle, move || Ok(ResponseExamples(client_handle.clone()))).unwrap();
    println!("Listening on http://{} with 1 thread.", serve.incoming_ref().local_addr());

    let h2 = handle.clone();
    handle.spawn(serve.for_each(move |conn| {
        h2.spawn(conn.map(|_| ()).map_err(|err| println!("serve error: {:?}", err)));
        Ok(())
    }).map_err(|_| ()));

    core.run(futures::future::empty::<(), ()>()).unwrap();
}
