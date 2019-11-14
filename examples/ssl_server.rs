use std::net::SocketAddr;
use futures_util::stream::StreamExt;

use hyper::{Body, Response, Request};
use hyper::service::{make_service_fn, service_fn};
use hyper::server::conn::Http;
use hyper::server::Builder;
use std::sync::Arc;
use tokio::net::TcpListener;

use native_tls;
use native_tls::Identity;
use tokio_tls;
use std::convert::Infallible;

async fn hello(_: Request<Body>) -> Result<Response<Body>, Infallible> {
    Ok(Response::new(Body::from("Hello World!")))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let addr: SocketAddr = ([127, 0, 0, 1], 8443).into();

    let cert = include_bytes!("./ssl_server.p12").to_vec();
    let cert_pass = "password";
    let cert = Identity::from_pkcs12(&cert, cert_pass)
      .expect("Could not decrypt p12 file");
    let tls_acceptor =
        tokio_tls::TlsAcceptor::from(
            native_tls::TlsAcceptor::builder(cert)
                .build()
                .expect("Could not create TLS acceptor.")
            );
    let _arc_acceptor = Arc::new(tls_acceptor);

    let service = make_service_fn(|_| {
        async {
            Ok::<_, Infallible>(service_fn(hello))
        }
    });

    let listener = TcpListener::bind(&addr).await.unwrap();
    let incoming = listener.incoming();
    let server = Builder
        ::new(hyper::server::accept::from_stream(incoming.filter_map(|socket| {
            async {
                match socket {
                    Ok(stream) => {
                        match _arc_acceptor.clone().accept(stream).await {
                            Ok(val) => Some(Ok::<_, hyper::Error>(val)),
                            Err(e) => {
                                println!("TLS error: {}", e);
                                None
                            }
                        }
                    },
                    Err(e) => {
                        println!("TCP socket error: {}", e);
                        None
                    }
                }
            }
        })), Http::new())
        .serve(service);

    server.await?;

    Ok(())
}
