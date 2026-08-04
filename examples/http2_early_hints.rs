//! HTTP/2 server demonstrating 103 Early Hints
//!
//! This example shows the recommended approach: 103 Early Hints.
//!
//! Run with:
//! ```
//! cargo run --example http2_early_hints --features full
//! ```

use std::convert::Infallible;
use std::fs;
use std::net::SocketAddr;
use std::time::Instant;

use bytes::Bytes;
use http::{Request, Response, StatusCode};
use http_body_util::Full;
use hyper::body::Incoming as IncomingBody;
use hyper::ext::early_hints_pusher;
use hyper::server::conn::http2;
use hyper::service::service_fn;
use tokio::net::TcpListener;
use tokio_rustls::rustls::{
    pki_types::{CertificateDer, PrivateKeyDer},
    ServerConfig,
};
use tokio_rustls::TlsAcceptor;

#[path = "../benches/support/mod.rs"]
mod support;
use support::{TokioExecutor, TokioIo};

/// Load certificates from provided files
fn load_certificates() -> Result<
    (Vec<CertificateDer<'static>>, PrivateKeyDer<'static>),
    Box<dyn std::error::Error + Send + Sync>,
> {
    // Read certificate file
    let cert_pem = fs::read_to_string("/tmp/cert.txt")?;

    // Parse certificate chain
    let mut certs = Vec::new();
    for cert in rustls_pemfile::certs(&mut cert_pem.as_bytes()) {
        certs.push(cert?);
    }

    // Read private key file
    let key_pem = fs::read_to_string("/tmp/key.txt")?;

    // Parse private key
    let mut key_reader = key_pem.as_bytes();
    let key =
        rustls_pemfile::private_key(&mut key_reader)?.ok_or("No private key found in key file")?;

    Ok((certs, key))
}

/// Generate a self-signed certificate for testing (fallback)
fn generate_self_signed_cert() -> (Vec<CertificateDer<'static>>, PrivateKeyDer<'static>) {
    use rcgen::{Certificate as RcgenCert, CertificateParams, DistinguishedName};

    let mut params = CertificateParams::new(vec!["localhost".to_string()]);
    params.distinguished_name = DistinguishedName::new();

    let cert = RcgenCert::from_params(params).unwrap();
    let cert_der = cert.serialize_der().unwrap();
    let private_key_der = cert.serialize_private_key_der();

    (
        vec![CertificateDer::from(cert_der)],
        PrivateKeyDer::try_from(private_key_der).unwrap(),
    )
}

/// HTTP service demonstrating 103 Early Hints
async fn handle_request(
    mut req: Request<IncomingBody>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let path = req.uri().path();
    println!("Received request: {} {}", req.method(), req.uri());

    // Handle static resources that we hinted about
    match path {
        "/css/critical.css" | "/css/layout.css" => {
            return Ok(Response::builder()
                .status(StatusCode::OK)
                .header("content-type", "text/css")
                .body(Full::new(Bytes::from("body { font-family: sans-serif; }")))
                .unwrap());
        }

        "/js/app.js" | "/js/vendor.js" => {
            return Ok(Response::builder()
                .status(StatusCode::OK)
                .header("content-type", "application/javascript")
                .body(Full::new(Bytes::from("console.log('loaded');")))
                .unwrap());
        }

        "/fonts/main.woff2" | "/fonts/icons.woff2" => {
            return Ok(Response::builder()
                .status(StatusCode::OK)
                .header("content-type", "font/woff2")
                .body(Full::new(Bytes::from(&b"WOFF2"[..])))
                .unwrap());
        }

        "/images/hero.webp" => {
            return Ok(Response::builder()
                .status(StatusCode::OK)
                .header("content-type", "image/webp")
                .body(Full::new(Bytes::from(&b"RIFF"[..])))
                .unwrap());
        }

        // Root path - serve HTML page with all the hinted resources
        "/" => {
            // Send 103 Early Hints using the early_hints_pusher API
            if let Ok(mut pusher) = early_hints_pusher(&mut req) {
                println!("Sending 103 Early Hints (all critical resources)");

                let start_time = Instant::now();

                let hints = Response::builder()
                    .status(StatusCode::EARLY_HINTS)
                    // Critical CSS (highest priority - render blocking)
                    .header("link", "</css/critical.css>; rel=preload; as=style")
                    .header("link", "</css/layout.css>; rel=preload; as=style")
                    // Critical JavaScript (high priority - interaction)
                    .header("link", "</js/app.js>; rel=preload; as=script")
                    .header("link", "</js/vendor.js>; rel=preload; as=script")
                    // Fonts (medium priority - text rendering)
                    .header(
                        "link",
                        "</fonts/main.woff2>; rel=preload; as=font; crossorigin",
                    )
                    .header(
                        "link",
                        "</fonts/icons.woff2>; rel=preload; as=font; crossorigin",
                    )
                    // Hero image (medium priority - above fold)
                    .header("link", "</images/hero.webp>; rel=preload; as=image")
                    // Metadata for tracking
                    .header("x-resource-count", "7")
                    .header("x-priority-order", "css,js,fonts,images")
                    .body(())
                    .unwrap();

                if let Err(e) = pusher.send_hints(hints).await {
                    eprintln!("Failed to send hints: {}", e);
                } else {
                    let send_duration = start_time.elapsed();
                    println!("103 Early Hints sent in: {:?}", send_duration);
                    println!("   7 resources hinted in single response");
                    println!("   Browser processes once, starts all preloads immediately");
                }

                // Simulate realistic server processing time
                println!("Processing request (simulating database queries, template rendering...)");
                tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
            }

            let html_content = r#"<!DOCTYPE html>
<html>
<head>
    <title>103 Early Hints Demo</title>
    <link rel="stylesheet" href="/css/critical.css">
    <link rel="stylesheet" href="/css/layout.css">
</head>
<body>
    <h1>HTTP/2 103 Early Hints</h1>
    <p>The resources above were hinted via 103 before this response arrived.</p>
    <script src="/js/app.js"></script>
</body>
</html>"#;

            return Ok(Response::builder()
                .status(StatusCode::OK)
                .header("content-type", "text/html")
                .body(Full::new(Bytes::from(html_content)))
                .unwrap());
        }

        // Default 404 handler
        _ => {
            return Ok(Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Full::new(Bytes::from("Not Found")))
                .unwrap());
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Initialize logging
    pretty_env_logger::init();

    let addr: SocketAddr = ([0, 0, 0, 0], 3000).into();

    // Load provided certificates or fallback to self-signed
    let (certs, key) = match load_certificates() {
        Ok((certs, key)) => {
            println!("Loaded certificates from /tmp/cert.txt and /tmp/key.txt");
            (certs, key)
        }
        Err(e) => {
            println!(
                "Failed to load provided certificates ({}), generating self-signed certificate...",
                e
            );
            generate_self_signed_cert()
        }
    };

    // Configure TLS
    let mut config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)?;

    // Enable HTTP/2
    config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

    let tls_acceptor = TlsAcceptor::from(std::sync::Arc::new(config));

    // Create TCP listener
    let listener = TcpListener::bind(addr).await?;
    println!("103 Early Hints Server listening on https://{}", addr);
    println!("Test: curl -k --http2 -v https://localhost:3000/");
    println!("Expected: 1 103 response + 1 final 200 response");
    println!("Benefits: Minimal browser overhead, maximum performance");

    loop {
        let (tcp_stream, _) = listener.accept().await?;
        let tls_acceptor = tls_acceptor.clone();

        tokio::spawn(async move {
            // Perform TLS handshake
            let tls_stream = match tls_acceptor.accept(tcp_stream).await {
                Ok(stream) => stream,
                Err(e) => {
                    eprintln!("TLS handshake failed: {}", e);
                    return;
                }
            };

            // Serve HTTP/2 connection with Early Hints support enabled
            let service = service_fn(handle_request);

            if let Err(e) = http2::Builder::new(TokioExecutor)
                .enable_informational() // Enable 103 Early Hints support
                .serve_connection(TokioIo::new(tls_stream), service)
                .await
            {
                eprintln!("HTTP/2 connection error: {}", e);
            }
        });
    }
}
