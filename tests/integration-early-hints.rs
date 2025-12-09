#![deny(warnings)]
#![cfg(feature = "http2")]

//! Integration tests for HTTP/2 103 Early Hints support.

use bytes::Bytes;
use http_body_util::Full;
use hyper::client::conn::http2::Builder;
use hyper::client::conn::informational::InformationalConfig;
use hyper::server::conn::http2::Builder as ServerBuilder;
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode};
use std::sync::{Arc, Mutex};
use tokio::net::{TcpListener, TcpStream};

#[path = "support/mod.rs"]
mod support;
use support::{TokioExecutor, TokioIo};

/// Basic end-to-end test: server sends 103 Early Hints, client receives it
/// via callback, then gets the final 200 response.
#[tokio::test]
async fn test_http2_103_early_hints_basic() {
    let _ = pretty_env_logger::try_init();

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    // Server: sends one 103 Early Hints then a 200 OK
    let server_handle = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let io = TokioIo::new(stream);

        let service = service_fn(|mut req: Request<hyper::body::Incoming>| async move {
            if let Ok(mut pusher) = hyper::ext::early_hints_pusher(&mut req) {
                let hints = Response::builder()
                    .status(StatusCode::EARLY_HINTS)
                    .header("link", "</style.css>; rel=preload; as=style")
                    .header("link", "</app.js>; rel=preload; as=script")
                    .body(())
                    .unwrap();
                let _ = pusher.send_hints(hints).await;
            }

            Ok::<_, hyper::Error>(
                Response::builder()
                    .status(StatusCode::OK)
                    .header("content-type", "text/html")
                    .body(Full::new(Bytes::from("<html>hello</html>")))
                    .unwrap(),
            )
        });

        ServerBuilder::new(TokioExecutor)
            .enable_informational()
            .serve_connection(io, service)
            .await
            .unwrap();
    });

    // Client: connects, registers informational callback, sends request
    let received = Arc::new(Mutex::new(Vec::<(u16, Vec<(String, String)>)>::new()));
    let received_clone = received.clone();

    let config = InformationalConfig::new().with_callback(move |response: Response<()>| {
        let headers: Vec<(String, String)> = response
            .headers()
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_str().unwrap().to_string()))
            .collect();
        received_clone
            .lock()
            .unwrap()
            .push((response.status().as_u16(), headers));
    });

    let stream = TcpStream::connect(addr).await.unwrap();
    let io = TokioIo::new(stream);

    let (mut sender, conn) = Builder::new(TokioExecutor)
        .informational_responses(config)
        .handshake(io)
        .await
        .unwrap();

    tokio::spawn(async move {
        let _ = conn.await;
    });

    let req = Request::builder()
        .uri("/")
        .body(Full::new(Bytes::new()))
        .unwrap();

    let response = sender.send_request(req).await.unwrap();

    // Verify final response
    assert_eq!(response.status(), StatusCode::OK);

    // Verify informational response was received
    let informational = received.lock().unwrap();
    assert_eq!(informational.len(), 1, "Expected one 103 response");
    assert_eq!(informational[0].0, 103);

    // Check that Link headers were received
    let link_headers: Vec<&String> = informational[0]
        .1
        .iter()
        .filter(|(k, _)| k == "link")
        .map(|(_, v)| v)
        .collect();
    assert!(!link_headers.is_empty(), "Expected Link headers in 103");

    server_handle.abort();
}

/// Test that multiple 103 Early Hints responses are received in sequence.
#[tokio::test]
async fn test_http2_103_early_hints_multiple() {
    let _ = pretty_env_logger::try_init();

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    // Server: sends two 103 Early Hints then a 200 OK
    let server_handle = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let io = TokioIo::new(stream);

        let service = service_fn(|mut req: Request<hyper::body::Incoming>| async move {
            if let Ok(mut pusher) = hyper::ext::early_hints_pusher(&mut req) {
                // First 103: CSS hints
                let hints1 = Response::builder()
                    .status(StatusCode::EARLY_HINTS)
                    .header("link", "</style.css>; rel=preload; as=style")
                    .body(())
                    .unwrap();
                let _ = pusher.send_hints(hints1).await;

                // Second 103: JS hints
                let hints2 = Response::builder()
                    .status(StatusCode::EARLY_HINTS)
                    .header("link", "</app.js>; rel=preload; as=script")
                    .body(())
                    .unwrap();
                let _ = pusher.send_hints(hints2).await;
            }

            Ok::<_, hyper::Error>(
                Response::builder()
                    .status(StatusCode::OK)
                    .body(Full::new(Bytes::from("done")))
                    .unwrap(),
            )
        });

        ServerBuilder::new(TokioExecutor)
            .enable_informational()
            .serve_connection(io, service)
            .await
            .unwrap();
    });

    // Client
    let received = Arc::new(Mutex::new(Vec::<(u16, Vec<(String, String)>)>::new()));
    let received_clone = received.clone();

    let config = InformationalConfig::new().with_callback(move |response: Response<()>| {
        let headers: Vec<(String, String)> = response
            .headers()
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_str().unwrap().to_string()))
            .collect();
        received_clone
            .lock()
            .unwrap()
            .push((response.status().as_u16(), headers));
    });

    let stream = TcpStream::connect(addr).await.unwrap();
    let io = TokioIo::new(stream);

    let (mut sender, conn) = Builder::new(TokioExecutor)
        .informational_responses(config)
        .handshake(io)
        .await
        .unwrap();

    tokio::spawn(async move {
        let _ = conn.await;
    });

    let req = Request::builder()
        .uri("/")
        .body(Full::new(Bytes::new()))
        .unwrap();

    let response = sender.send_request(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Verify both 103 responses were received
    let informational = received.lock().unwrap();
    assert_eq!(informational.len(), 2, "Expected two 103 responses");
    assert_eq!(informational[0].0, 103);
    assert_eq!(informational[1].0, 103);

    server_handle.abort();
}
