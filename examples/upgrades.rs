#![feature(async_await)]
#![deny(warnings)]

// Note: `hyper::upgrade` docs link to this upgrade.
use std::str;

use tokio::sync::oneshot;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use hyper::{Body, Client, Request, Response, Server, StatusCode};
use hyper::header::{UPGRADE, HeaderValue};
use hyper::service::{make_service_fn, service_fn};
use hyper::upgrade::Upgraded;
use std::net::SocketAddr;

// A simple type alias so as to DRY.
type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// Handle server-side I/O after HTTP upgraded.
async fn server_upgraded_io(mut upgraded: Upgraded) -> Result<()> {
    // we have an upgraded connection that we can read and
    // write on directly.
    //
    // since we completely control this example, we know exactly
    // how many bytes the client will write, so just read exact...
    let mut vec = vec![0; 7];
    upgraded.read_exact(&mut vec).await?;
    println!("server[foobar] recv: {:?}", str::from_utf8(&vec));

    // and now write back the server 'foobar' protocol's
    // response...
    upgraded.write_all(b"barr=foo").await?;
    println!("server[foobar] sent");
    Ok(())
}

/// Our server HTTP handler to initiate HTTP upgrades.
async fn server_upgrade(req: Request<Body>) -> Result<Response<Body>> {
    let mut res = Response::new(Body::empty());

    // Send a 400 to any request that doesn't have
    // an `Upgrade` header.
    if !req.headers().contains_key(UPGRADE) {
        *res.status_mut() = StatusCode::BAD_REQUEST;
        return Ok(res);
    }

    // Setup a future that will eventually receive the upgraded
    // connection and talk a new protocol, and spawn the future
    // into the runtime.
    //
    // Note: This can't possibly be fulfilled until the 101 response
    // is returned below, so it's better to spawn this future instead
    // waiting for it to complete to then return a response.
    hyper::rt::spawn(async move {
        match req.into_body().on_upgrade().await {
            Ok(upgraded) => {
                if let Err(e) = server_upgraded_io(upgraded).await {
                    eprintln!("server foobar io error: {}", e)
                };
            }
            Err(e) => eprintln!("upgrade error: {}", e)
        }
    });

    // Now return a 101 Response saying we agree to the upgrade to some
    // made-up 'foobar' protocol.
    *res.status_mut() = StatusCode::SWITCHING_PROTOCOLS;
    res.headers_mut().insert(UPGRADE, HeaderValue::from_static("foobar"));
    Ok(res)
}

/// Handle client-side I/O after HTTP upgraded.
async fn client_upgraded_io(mut upgraded: Upgraded) -> Result<()> {
    // We've gotten an upgraded connection that we can read
    // and write directly on. Let's start out 'foobar' protocol.
    upgraded.write_all(b"foo=bar").await?;
    println!("client[foobar] sent");

    let mut vec = Vec::new();
    upgraded.read_to_end(&mut vec).await?;
    println!("client[foobar] recv: {:?}", str::from_utf8(&vec));

    Ok(())
}

/// Our client HTTP handler to initiate HTTP upgrades.
async fn client_upgrade_request(addr: SocketAddr) -> Result<()> {
    let req = Request::builder()
        .uri(format!("http://{}/", addr))
        .header(UPGRADE, "foobar")
        .body(Body::empty())
        .unwrap();

    let res = Client::new().request(req).await?;
    if res.status() != StatusCode::SWITCHING_PROTOCOLS {
        panic!("Our server didn't upgrade: {}", res.status());
    }

    match res.into_body().on_upgrade().await {
        Ok(upgraded) => {
            if let Err(e) = client_upgraded_io(upgraded).await {
                eprintln!("client foobar io error: {}", e)
            };
        }
        Err(e) => eprintln!("upgrade error: {}", e)
    }

    Ok(())
}

#[tokio::main]
async fn main() {
    // For this example, we just make a server and our own client to talk to
    // it, so the exact port isn't important. Instead, let the OS give us an
    // unused port.
    let addr = ([127, 0, 0, 1], 0).into();

    let make_service = make_service_fn(|_| async {
        Ok::<_, hyper::Error>(service_fn(server_upgrade))
    });

    let server = Server::bind(&addr)
        .serve(make_service);

    // We need the assigned address for the client to send it messages.
    let addr = server.local_addr();

    // For this example, a oneshot is used to signal that after 1 request,
    // the server should be shutdown.
    let (tx, rx) = oneshot::channel::<()>();
    let server = server
        .with_graceful_shutdown(async {
            rx.await.ok();
        });

    // Spawn server on the default executor,
    // which is usually a thread-pool from tokio default runtime.
    hyper::rt::spawn(async {
        if let Err(e) = server.await {
            eprintln!("server error: {}", e);
        }
    });

    // Client requests a HTTP connection upgrade.
    let request = client_upgrade_request(addr.clone());
    if let Err(e) = request.await {
        eprintln!("client error: {}", e);
    }

    // Complete the oneshot so that the server stops
    // listening and the process can close down.
    let _ = tx.send(());
}
