#![feature(test)]
#![deny(warnings)]

extern crate test;

use tokio::net::TcpListener;
use tokio::runtime::current_thread::Runtime;
use hyper::client::connect::{Destination, HttpConnector};
use hyper::service::Service;
use http::Uri;

#[bench]
fn http_connector(b: &mut test::Bencher) {
    let _ = pretty_env_logger::try_init();
    let mut rt = Runtime::new().unwrap();
    let mut listener = rt.block_on(TcpListener::bind("127.0.0.1:0")).expect("bind");
    let addr = listener.local_addr().expect("local_addr");
    let uri: Uri = format!("http://{}/", addr).parse().expect("uri parse");
    let dst = Destination::try_from_uri(uri).expect("destination");
    let mut connector = HttpConnector::new();

    rt.spawn(async move {
        loop {
            let _ = listener.accept().await;
        }
    });


    b.iter(|| {
        rt.block_on(async {
            connector.call(dst.clone()).await.expect("connect");
        });
    });
}
