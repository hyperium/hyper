extern crate pretty_env_logger;

use std::thread;
use std::time::Duration;

use futures::Async;
use futures::future::poll_fn;
use futures::executor::block_on;
use tokio::executor::thread_pool::{Builder as ThreadPoolBuilder};

use mock::MockConnector;
use super::*;

#[test]
fn retryable_request() {
    let _ = pretty_env_logger::try_init();

    let executor = ThreadPoolBuilder::new().pool_size(1).build();
    let mut connector = MockConnector::new();

    let sock1 = connector.mock("http://mock.local");
    let sock2 = connector.mock("http://mock.local");

    let client = Client::configure()
        .connector(connector)
        .build();

    {

        let req = Request::builder()
            .uri("http://mock.local/a")
            .body(Default::default())
            .unwrap();
        let res1 = client.request(req).with_executor(executor.sender().clone());
        let srv1 = poll_fn(|cx| {
            try_ready!(sock1.read(cx, &mut [0u8; 512]));
            try_ready!(sock1.write(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n"));
            Ok(Async::Ready(()))
        });
        block_on(res1.join(srv1)).expect("res1");
    }
    drop(sock1);

    let req = Request::builder()
        .uri("http://mock.local/b")
        .body(Default::default())
        .unwrap();
    let res2 = client.request(req).with_executor(executor.sender().clone())
        .map(|res| {
            assert_eq!(res.status().as_u16(), 222);
        });
    let srv2 = poll_fn(|cx| {
        try_ready!(sock2.read(cx, &mut [0u8; 512]));
        try_ready!(sock2.write(b"HTTP/1.1 222 OK\r\nContent-Length: 0\r\n\r\n"));
        Ok(Async::Ready(()))
    });

    block_on(res2.join(srv2)).expect("res2");
}

#[test]
fn conn_reset_after_write() {
    let _ = pretty_env_logger::try_init();

    let executor = ThreadPoolBuilder::new().pool_size(1).build();
    let mut connector = MockConnector::new();

    let sock1 = connector.mock("http://mock.local");

    let client = Client::configure()
        .connector(connector)
        .build();

    {
        let req = Request::builder()
            .uri("http://mock.local/a")
            //TODO: remove this header when auto lengths are fixed
            .header("content-length", "0")
            .body(Default::default())
            .unwrap();
        let res1 = client.request(req).with_executor(executor.sender().clone());
        let srv1 = poll_fn(|cx| {
            try_ready!(sock1.read(cx, &mut [0u8; 512]));
            try_ready!(sock1.write(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n"));
            Ok(Async::Ready(()))
        });
        block_on(res1.join(srv1)).expect("res1");
    }

    // sleep to allow some time for the connection to return to the pool
    thread::sleep(Duration::from_secs(1));

    let req = Request::builder()
        .uri("http://mock.local/a")
        .body(Default::default())
        .unwrap();
    let res2 = client.request(req).with_executor(executor.sender().clone());
    let mut sock1 = Some(sock1);
    let srv2 = poll_fn(|cx| {
        // We purposefully keep the socket open until the client
        // has written the second request, and THEN disconnect.
        //
        // Not because we expect servers to be jerks, but to trigger
        // state where we write on an assumedly good connetion, and
        // only reset the close AFTER we wrote bytes.
        try_ready!(sock1.as_mut().unwrap().read(cx, &mut [0u8; 512]));
        sock1.take();
        Ok(Async::Ready(()))
    });
    let err = block_on(res2.join(srv2)).expect_err("res2");
    match err {
        ::Error::Incomplete => (),
        other => panic!("expected Incomplete, found {:?}", other)
    }
}
