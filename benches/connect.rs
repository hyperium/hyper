#![feature(test)]
#![deny(warnings)]

extern crate test;

use http::Uri;
use hyper::client::connect::HttpConnector;
use hyper::service::Service;
use std::net::SocketAddr;
use tokio::net::TcpListener;

#[bench]
fn http_connector(b: &mut test::Bencher) {
    let _ = pretty_env_logger::try_init();
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("rt build");
    let listener = rt
        .block_on(TcpListener::bind(&SocketAddr::from(([127, 0, 0, 1], 0))))
        .expect("bind");
    let addr = listener.local_addr().expect("local_addr");
    let dst: Uri = format!("http://{}/", addr).parse().expect("uri parse");
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
