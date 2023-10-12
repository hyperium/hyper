#![deny(warnings)]
#![warn(rust_2018_idioms)]

use std::net::SocketAddr;

use bytes::Bytes;
use futures_util::future::join;
use http_body_util::Full;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response};
use tokio::net::TcpListener;

#[path = "../benches/support/mod.rs"]
mod support;
use support::TokioIo;

static INDEX1: &[u8] = b"The 1st service!";
static INDEX2: &[u8] = b"The 2nd service!";

async fn index1(_: Request<hyper::body::Incoming>) -> Result<Response<Full<Bytes>>, hyper::Error> {
    Ok(Response::new(Full::new(Bytes::from(INDEX1))))
}

async fn index2(_: Request<hyper::body::Incoming>) -> Result<Response<Full<Bytes>>, hyper::Error> {
    Ok(Response::new(Full::new(Bytes::from(INDEX2))))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    pretty_env_logger::init();

    let addr1: SocketAddr = ([127, 0, 0, 1], 1337).into();
    let addr2: SocketAddr = ([127, 0, 0, 1], 1338).into();

    let srv1 = async move {
        let listener = TcpListener::bind(addr1).await.unwrap();
        loop {
            let (stream, _) = listener.accept().await.unwrap();
            let io = TokioIo::new(stream);

            tokio::task::spawn(async move {
                if let Err(err) = http1::Builder::new()
                    .serve_connection(io, service_fn(index1))
                    .await
                {
                    println!("Error serving connection: {:?}", err);
                }
            });
        }
    };

    let srv2 = async move {
        let listener = TcpListener::bind(addr2).await.unwrap();
        loop {
            let (stream, _) = listener.accept().await.unwrap();
            let io = TokioIo::new(stream);

            tokio::task::spawn(async move {
                if let Err(err) = http1::Builder::new()
                    .serve_connection(io, service_fn(index2))
                    .await
                {
                    println!("Error serving connection: {:?}", err);
                }
            });
        }
    };

    println!("Listening on http://{} and http://{}", addr1, addr2);

    let _ret = join(srv1, srv2).await;

    Ok(())
}
