#![deny(warnings)]

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use hyper::service::Service;
use hyper::{Body, Request, Response};
use tokio::net::TcpStream;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
    pretty_env_logger::init();

    let uri = "http://127.0.0.1:8080".parse::<http::Uri>()?;

    let mut svc = Connector;

    let body = Body::empty();

    let req = Request::get(uri).body(body)?;
    let res = svc.call(req).await?;

    println!("RESPONSE={:?}", res);

    Ok(())
}

struct Connector;

impl Service<Request<Body>> for Connector {
    type Response = Response<Body>;
    type Error = Box<dyn std::error::Error + Send + Sync + 'static>;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> std::task::Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        Box::pin(async move {
            let host = req.uri().host().expect("no host in uri");
            let port = req.uri().port_u16().expect("no port in uri");

            let stream = TcpStream::connect(format!("{}:{}", host, port)).await?;

            let (mut sender, conn) = hyper::client::conn::http1::handshake(stream).await?;

            tokio::task::spawn(async move {
                if let Err(err) = conn.await {
                    println!("Connection error: {:?}", err);
                }
            });

            let res = sender.send_request(req).await?;
            Ok(res)
        })
    }
}
