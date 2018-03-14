#![deny(warnings)]
extern crate futures;
extern crate hyper;
extern crate pretty_env_logger;
extern crate tokio_core;

use futures::{Future, Stream};

use hyper::{Body, Chunk, Client, Method, Request, Response, StatusCode};
use hyper::server::{Http, Service};

#[allow(unused)]
use std::ascii::AsciiExt;

static NOTFOUND: &[u8] = b"Not Found";
static URL: &str = "http://127.0.0.1:1337/web_api";
static INDEX: &[u8] = b"<a href=\"test.html\">test.html</a>";
static LOWERCASE: &[u8] = b"i am a lower case string";

struct ResponseExamples(tokio_core::reactor::Handle);

impl Service for ResponseExamples {
    type Request = Request<Body>;
    type Response = Response<Body>;
    type Error = hyper::Error;
    type Future = Box<Future<Item = Self::Response, Error = Self::Error>>;

    fn call(&self, req: Self::Request) -> Self::Future {
        match (req.method(), req.uri().path()) {
            (&Method::GET, "/") | (&Method::GET, "/index.html") => {
                let body = Body::from(INDEX);
                Box::new(futures::future::ok(Response::new(body)))
            },
            (&Method::GET, "/test.html") => {
                // Run a web query against the web api below
                let client = Client::configure().build(&self.0);
                let req = Request::builder()
                    .method(Method::POST)
                    .uri(URL)
                    .body(LOWERCASE.into())
                    .unwrap();
                let web_res_future = client.request(req);

                Box::new(web_res_future.map(|web_res| {
                    let body = Body::wrap_stream(web_res.into_body().into_stream().map(|b| {
                        Chunk::from(format!("before: '{:?}'<br>after: '{:?}'",
                                            std::str::from_utf8(LOWERCASE).unwrap(),
                                            std::str::from_utf8(&b).unwrap()))
                    }));
                    Response::new(body)
                }))
            },
            (&Method::POST, "/web_api") => {
                // A web api to run against. Simple upcasing of the body.
                let body = Body::wrap_stream(req.into_body().into_stream().map(|chunk| {
                    let upper = chunk.iter().map(|byte| byte.to_ascii_uppercase())
                        .collect::<Vec<u8>>();
                    Chunk::from(upper)
                }));
                Box::new(futures::future::ok(Response::new(body)))
            },
            _ => {
                let body = Body::from(NOTFOUND);
                Box::new(futures::future::ok(Response::builder()
                                             .status(StatusCode::NOT_FOUND)
                                             .body(body)
                                             .unwrap()))
            }
        }
    }

}


fn main() {
    pretty_env_logger::init();
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
