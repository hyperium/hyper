//! MPTCP is a new protocol added in the Linux Kernel since 5.6.
//! The goal of MPTCP is to support multiple path (cf: ADSL & LTE).
//! This allow for aggregation of links's bandwidth and for resiliency
//! if any of the link fails.
//! Even if MPTCP is added in the Linux Kernel since 5.6, you may
//! still need to enable it manually using:
//! `sysctl net.mptcp.enabled=1`
//! After that your host should be compatible with MPTCP.
//!
//! Note: a socket running with MPTCP is still compatible with TCP.
#![deny(warnings)]

use std::convert::Infallible;
use std::net::SocketAddr;

use bytes::Bytes;
use http_body_util::Full;
use hyper::server::conn::Http;
use hyper::service::service_fn;
use hyper::{Recv, Request, Response};
use socket2::{Domain, Protocol, Socket, Type};
use tokio::net::TcpSocket;

async fn hello(_: Request<Recv>) -> Result<Response<Full<Bytes>>, Infallible> {
    Ok(Response::new(Full::new(Bytes::from("Hello MPTCP!"))))
}

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    pretty_env_logger::init();

    let addr: SocketAddr = ([127, 0, 0, 1], 3000).into();

    // Create the MPTCP capable socket but allow for a fallback to TCP if the host
    // does not support MPTCP.
    let socket = match Socket::new(
        Domain::for_address(addr),
        Type::STREAM,
        Some(Protocol::MPTCP),
    ) {
        Ok(sock) => sock,
        Err(_) => {
            println!("The host does not support MPTCP, fallback to TCP");
            Socket::new(Domain::for_address(addr), Type::STREAM, Some(Protocol::TCP))?
        }
    };
    // Set common options on the socket as we created it by hand.
    socket.set_nonblocking(true)?;
    socket.set_reuse_address(true)?;
    socket.bind(&addr.into())?;

    // Transform our Socket2 into a TcpSocket->TcpListener (tokio::net)
    let listener = TcpSocket::from_std_stream(socket.into()).listen(1024)?;
    println!("Listening on http://{}", addr);
    loop {
        let (stream, _) = listener.accept().await?;

        tokio::task::spawn(async move {
            if let Err(err) = Http::new()
                .serve_connection(stream, service_fn(hello))
                .await
            {
                println!("Error serving connection: {:?}", err);
            }
        });
    }
}
