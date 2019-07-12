// Note: `hyper::upgrade` docs link to this upgrade.
extern crate futures;
extern crate hyper;
extern crate tokio;

use std::str;

use futures::sync::oneshot;

use hyper::{Body, Client, Request, Response, Server, StatusCode};
use hyper::header::{UPGRADE, HeaderValue};
use hyper::rt::{self, Future};
use hyper::service::service_fn_ok;

/// Our server HTTP handler to initiate HTTP upgrades.
fn server_upgrade(req: Request<Body>) -> Response<Body> {
    let mut res = Response::new(Body::empty());

    // Send a 400 to any request that doesn't have
    // an `Upgrade` header.
    if !req.headers().contains_key(UPGRADE) {
        *res.status_mut() = StatusCode::BAD_REQUEST;
        return res;
    }

    // Setup a future that will eventually receive the upgraded
    // connection and talk a new protocol, and spawn the future
    // into the runtime.
    //
    // Note: This can't possibly be fulfilled until the 101 response
    // is returned below, so it's better to spawn this future instead
    // waiting for it to complete to then return a response.
    let on_upgrade = req
        .into_body()
        .on_upgrade()
        .map_err(|err| eprintln!("upgrade error: {}", err))
        .and_then(|upgraded| {
            // We have an upgraded connection that we can read and
            // write on directly.
            //
            // Since we completely control this example, we know exactly
            // how many bytes the client will write, so just read exact...
            tokio::io::read_exact(upgraded, vec![0; 7])
                .and_then(|(upgraded, vec)| {
                    println!("server[foobar] recv: {:?}", str::from_utf8(&vec));

                    // And now write back the server 'foobar' protocol's
                    // response...
                    tokio::io::write_all(upgraded, b"bar=foo")
                })
                .map(|_| println!("server[foobar] sent"))
                .map_err(|e| eprintln!("server foobar io error: {}", e))
        });

    rt::spawn(on_upgrade);


    // Now return a 101 Response saying we agree to the upgrade to some
    // made-up 'foobar' protocol.
    *res.status_mut() = StatusCode::SWITCHING_PROTOCOLS;
    res.headers_mut().insert(UPGRADE, HeaderValue::from_static("foobar"));
    res
}

fn main() {
    // For this example, we just make a server and our own client to talk to
    // it, so the exact port isn't important. Instead, let the OS give us an
    // unused port.
    let addr = ([127, 0, 0, 1], 0).into();

    let server = Server::bind(&addr)
        .serve(|| service_fn_ok(server_upgrade));

    // We need the assigned address for the client to send it messages.
    let addr = server.local_addr();


    // For this example, a oneshot is used to signal that after 1 request,
    // the server should be shutdown.
    let (tx, rx) = oneshot::channel();

    let server = server
        .map_err(|e| eprintln!("server error: {}", e))
        .select2(rx)
        .then(|_| Ok(()));

    rt::run(rt::lazy(move || {
        rt::spawn(server);

        let req = Request::builder()
            .uri(format!("http://{}/", addr))
            .header(UPGRADE, "foobar")
            .body(Body::empty())
            .unwrap();

        Client::new()
            .request(req)
            .and_then(|res| {
                if res.status() != StatusCode::SWITCHING_PROTOCOLS {
                    panic!("Our server didn't upgrade: {}", res.status());
                }

                res
                    .into_body()
                    .on_upgrade()
            })
            .map_err(|e| eprintln!("client error: {}", e))
            .and_then(|upgraded| {
                // We've gotten an upgraded connection that we can read
                // and write directly on. Let's start out 'foobar' protocol.
                tokio::io::write_all(upgraded, b"foo=bar")
                    .and_then(|(upgraded, _)| {
                        println!("client[foobar] sent");
                        tokio::io::read_to_end(upgraded, Vec::new())
                    })
                    .map(|(_upgraded, vec)| {
                        println!("client[foobar] recv: {:?}", str::from_utf8(&vec));


                        // Complete the oneshot so that the server stops
                        // listening and the process can close down.
                        let _ = tx.send(());
                    })
                    .map_err(|e| eprintln!("client foobar io error: {}", e))
            })
    }));
}
