#![deny(warnings)]

use std::net::SocketAddr;

use hyper::server::conn::Http;
use tokio::net::TcpListener;

use bytes::Bytes;
use http_body_util::Full;
use hyper::service::service_fn;
use hyper::{Body, Method, Request, Response, Result, StatusCode};

static INDEX: &str = "examples/send_file_index.html";
static NOTFOUND: &[u8] = b"Not Found";

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    pretty_env_logger::init();

    let addr: SocketAddr = "127.0.0.1:1337".parse().unwrap();

    let listener = TcpListener::bind(addr).await?;
    println!("Listening on http://{}", addr);

    loop {
        let (stream, _) = listener.accept().await?;

        tokio::task::spawn(async move {
            if let Err(err) = Http::new()
                .serve_connection(stream, service_fn(response_examples))
                .await
            {
                println!("Failed to serve connection: {:?}", err);
            }
        });
    }
}

async fn response_examples(req: Request<Body>) -> Result<Response<Full<Bytes>>> {
    match (req.method(), req.uri().path()) {
        (&Method::GET, "/") | (&Method::GET, "/index.html") => simple_file_send(INDEX).await,
        (&Method::GET, "/no_file.html") => {
            // Test what happens when file cannot be be found
            simple_file_send("this_file_should_not_exist.html").await
        }
        _ => Ok(not_found()),
    }
}

/// HTTP status code 404
fn not_found() -> Response<Full<Bytes>> {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Full::new(NOTFOUND.into()))
        .unwrap()
}

async fn simple_file_send(filename: &str) -> Result<Response<Full<Bytes>>> {
    if let Ok(contents) = tokio::fs::read(filename).await {
        let body = contents.into();
        return Ok(Response::new(Full::new(body)));
    }

    Ok(not_found())
}
