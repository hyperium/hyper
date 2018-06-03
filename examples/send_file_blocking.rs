#![deny(warnings)]
extern crate futures;
extern crate hyper;
extern crate pretty_env_logger;
extern crate tokio_threadpool;

use futures::{future, Future};

use hyper::{Body, Method, Request, Response, Server, StatusCode};
use hyper::service::service_fn;

use std::fs::File;
use std::io::{self, copy/*, Read*/};

use tokio_threadpool::blocking;

static NOTFOUND: &[u8] = b"Not Found";
static INDEX: &str = "examples/send_file_index.html";


fn main() {
    pretty_env_logger::init();

    let addr = "127.0.0.1:1337".parse().unwrap();

    let server = Server::bind(&addr)
        .serve(|| service_fn(response_examples))
        .map_err(|e| eprintln!("server error: {}", e));

    println!("Listening on http://{}", addr);

    hyper::rt::run(server);
}

type ResponseFuture = Box<Future<Item=Response<Body>, Error=io::Error> + Send>;

fn response_examples(req: Request<Body>) -> ResponseFuture {
    match (req.method(), req.uri().path()) {
        (&Method::GET, "/") | (&Method::GET, "/index.html") | (&Method::GET, "/big_file.html") => {
            simple_file_send(INDEX)
        },
        (&Method::GET, "/no_file.html") => {
            // Test what happens when file cannot be be found
            simple_file_send("this_file_should_not_exist.html")
        },
        _ => {
            Box::new(future::ok(Response::builder()
                                .status(StatusCode::NOT_FOUND)
                                .body(Body::empty())
                                .unwrap()))
        }
    }

}

fn simple_file_send(f: &str) -> ResponseFuture {
    // Serve a file by reading it entirely into memory. As a result
    // this is limited to serving small files, but it is somewhat
    // simpler with a little less overhead.
    //
    // Wrap in tokio_threadpool::blocking to tell the ThreadPool
    // that the call to read_file_blocking will block.
    let filename = f.to_string(); // we need to copy for lifetime issues
    Box::new(
        future::poll_fn(move || {
            blocking(|| read_file_blocking(&filename))
            .map_err(|e| ::std::io::Error::new(::std::io::ErrorKind::Other, e))
        })
    )
}

fn read_file_blocking(filename: &str) -> Response<Body> {
    // Open the file and read it using blocking read.
    let mut file = match File::open(filename) {
        Ok(f) => f,
        Err(_) => {
            return Response::builder()
                   .status(StatusCode::NOT_FOUND)
                   .body(NOTFOUND.into())
                   .unwrap();
        }
    };
    let mut buf: Vec<u8> = Vec::new();
    match copy(&mut file, &mut buf) {
        Ok(_) => {
            Response::new(buf.into())
        },
        Err(_) => {
            Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Body::empty())
            .unwrap()
        },
    }
}
