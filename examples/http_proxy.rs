#![deny(warnings)]

use std::net::ToSocketAddrs;

use futures_util::future;
use futures_util::try_future::try_join;

use hyper::service::{make_service_fn, service_fn};
use hyper::upgrade::Upgraded;
use hyper::{Body, Client, Error, Method, Request, Response, Server};
use hyper::server::conn::AddrStream;

use tokio::prelude::*;
use tokio_net::tcp::TcpStream;

// Create a TCP connection to host:port, build a tunnel between the connection and
// the upgraded connection
//
// refer to https://github.com/tokio-rs/tokio/blob/master/tokio/examples_old/proxy.rs
async fn tunnel(upgraded: Upgraded, host: String, port: u16) -> std::io::Result<()> {
    // Connect to remote server
    let remote_addr = format!("{}:{}", host, port);
    let remote_addr: Vec<_> = remote_addr
        .to_socket_addrs()
        .expect("Unable to resolve domain")
        .collect();
    let server = TcpStream::connect(&remote_addr[0]);

    // Proxying data
    let amounts = {
        let (mut server_reader, mut server_writer) = server.await?.split();
        let parts = upgraded.downcast::<AddrStream>().unwrap();
        let (mut client_reader, mut client_writer) = parts.io.into_inner().split();

        server_writer.write_all(parts.read_buf.as_ref()).await?;
        let client_to_server = client_reader.copy(&mut server_writer);
        let server_to_client = server_reader.copy(&mut client_writer);

        try_join(client_to_server, server_to_client).await
    };

    // Print message when done
    match amounts {
        Ok((from_client, from_server)) => {
            println!("client wrote {} bytes and received {} bytes", from_client, from_server);
        }
        Err(e) => {
            println!("tunnel error: {}", e);
        }
    };
    Ok(())
}

// To try this example:
// 1. cargo run --example http_proxy
// 2. config http_proxy in command line
//    $ export http_proxy=http://127.0.0.1:8100
//    $ export https_proxy=http://127.0.0.1:8100
// 3. send requests
//    $ curl -i https://www.some_domain.com/
#[tokio::main]
pub async fn main() {
    let addr = ([127, 0, 0, 1], 8100).into();
    let client_main = Client::new();

    let make_service = make_service_fn(move |_| {
        let client = client_main.clone();
        async move {
            Ok::<_, Error>(service_fn(move |req: Request<Body>| {
                println!("req: {:?}", req);

                if Method::CONNECT == req.method() {
                    // Recieved an HTTP request like:
                    // ```
                    // CONNECT www.domain.com:443 HTTP/1.1
                    // Host: www.domain.com:443
                    // Proxy-Connection: Keep-Alive
                    // ```
                    //
                    // When HTTP method is CONNECT we should return an empty body
                    // then we can eventually upgrade the connection and talk a new protocol.
                    //
                    // Note: only after client recieved an empty body with STATUS_OK can the
                    // connection be upgraded, so we can't return a response inside
                    // `on_upgrade` future.
                    let host = req.uri().host().unwrap().to_string();
                    let port = req.uri().port_u16().unwrap();
                    hyper::rt::spawn(async move {
                        match req.into_body().on_upgrade().await {
                            Ok(upgraded) => {
                                if let Err(e) = tunnel(upgraded, host, port).await {
                                    eprintln!("server io error: {}", e);
                                };
                            }
                            Err(e) => eprintln!("upgrade error: {}", e),
                        }
                    });


                    future::Either::Left(future::ok(Response::new(Body::empty())))
                } else {
                    future::Either::Right(client.request(req))
                }
            }))
        }
    });

    let server = Server::bind(&addr).serve(make_service);

    println!("Listening on http://{}", addr);

    if let Err(e) = server.await {
        eprintln!("server error: {}", e);
    }
}