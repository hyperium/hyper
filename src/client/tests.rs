extern crate pretty_env_logger;

use futures::Async;
use futures::future::poll_fn;
use tokio::reactor::Core;

use mock::MockConnector;
use super::*;

#[test]
fn retryable_request() {
    let _ = pretty_env_logger::try_init();
    let mut core = Core::new().unwrap();

    let mut connector = MockConnector::new();

    let sock1 = connector.mock("http://mock.local/a");
    let sock2 = connector.mock("http://mock.local/b");

    let client = Client::configure()
        .connector(connector)
        .build(&core.handle());


    {
        let res1 = client.get("http://mock.local/a".parse().unwrap());
        let srv1 = poll_fn(|| {
            try_ready!(sock1.read(&mut [0u8; 512]));
            try_ready!(sock1.write(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n"));
            Ok(Async::Ready(()))
        });
        core.run(res1.join(srv1)).expect("res1");
    }
    drop(sock1);

    let res2 = client.get("http://mock.local/b".parse().unwrap())
        .map(|res| {
            assert_eq!(res.status().as_u16(), 222);
        });
    let srv2 = poll_fn(|| {
        try_ready!(sock2.read(&mut [0u8; 512]));
        try_ready!(sock2.write(b"HTTP/1.1 222 OK\r\nContent-Length: 0\r\n\r\n"));
        Ok(Async::Ready(()))
    });

    core.run(res2.join(srv2)).expect("res2");
}
