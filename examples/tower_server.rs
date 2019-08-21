#![deny(warnings)]

use hyper::{Body, Request, Response, Server};
use tower_service::Service;
use futures_util::future;
use std::task::{Context, Poll};

const ROOT: &'static str = "/";

#[derive(Debug)]
pub struct Svc;

impl Service<Request<Body>> for Svc {
    type Response = Response<Body>;
    type Error = hyper::Error;
    type Future = future::Ready<Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Ok(()).into()
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let mut rsp = Response::builder();

        let uri = req.uri();
        if uri.path() != ROOT {
            let body = Body::from(Vec::new());
            let rsp = rsp.status(404).body(body).unwrap();
            return future::ok(rsp);
        }

        let body = Body::from(Vec::from(&b"heyo!"[..]));
        let rsp = rsp.status(200).body(body).unwrap();
        future::ok(rsp)
    }
}

pub struct MakeSvc;

impl<T> Service<T> for MakeSvc {
    type Response = Svc;
    type Error = std::io::Error;
    type Future = future::Ready<Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Ok(()).into()
    }

    fn call(&mut self, _: T) -> Self::Future {
        future::ok(Svc)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    pretty_env_logger::init();

    let addr = "127.0.0.1:1337".parse().unwrap();


    let server = Server::bind(&addr)
        .serve(MakeSvc);

    println!("Listening on http://{}", addr);

    server.await?;

    Ok(())
}
