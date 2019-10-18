#![deny(warnings)]

use hyper::client::service::Connect;
use hyper::client::conn::Builder;
use hyper::client::connect::HttpConnector;
use hyper::service::Service;
use hyper::{Body, Request};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    pretty_env_logger::init();

    let mut mk_svc = Connect::new(HttpConnector::new(), Builder::new());

    let uri = "http://127.0.0.1:8080".parse::<http::Uri>()?;


    let mut svc = mk_svc.call(uri.clone()).await?;

    let body = Body::empty();

    let req = Request::get(uri).body(body)?;
    let res = svc.call(req).await?;

    println!("RESPONSE={:?}", res);

    Ok(())
}
