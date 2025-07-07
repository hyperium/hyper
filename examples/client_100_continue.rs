#![deny(warnings)]
#![warn(rust_2018_idioms)]
use std::env;
use std::pin::Pin;
use std::task::Poll;

use bytes::Bytes;
use futures_util::TryFuture;
use http_body::Body;
use http_body_util::BodyExt;
use http_body_util::Full;
use hyper::ext::on_informational;
use hyper::Request;
use pin_project_lite::pin_project;
use tokio::io::stdout;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;

#[path = "../benches/support/mod.rs"]
mod support;
use support::TokioIo;

// A simple type alias so as to DRY.
type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

pin_project! {
    /// A body that delays processing until a signal is received.
    struct DelayedBody<B> {
        #[pin]
        inner: B,
        rx: Option<oneshot::Receiver<()>>,
    }
}

impl<B> Body for DelayedBody<B>
where
    B: Body,
{
    type Data = B::Data;
    type Error = B::Error;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<std::result::Result<http_body::Frame<Self::Data>, Self::Error>>> {
        let this = self.project();

        // Check if we have a receiver and poll it (only process once)
        if let Some(ref mut rx) = this.rx.as_mut() {
            match Pin::new(rx).try_poll(cx) {
                Poll::Ready(Ok(())) => {
                    println!("Received signal to start processing body.");
                    *this.rx = None;
                    this.inner.poll_frame(cx)
                }
                Poll::Ready(Err(_)) => {
                    println!("Sender dropped, proceeding without signal.");
                    *this.rx = None;
                    this.inner.poll_frame(cx)
                }
                Poll::Pending => {
                    // No signal yet, return pending
                    Poll::Pending
                }
            }
        } else {
            this.inner.poll_frame(cx)
        }
    }
}

/// To try this example:
/// 1. Start the server in one terminal:
///    $ cargo run --example client_100_continue --features="full" -- --server 8080
/// 2. Run the client in another terminal:
///    $ cargo run --example client_100_continue --features="full" -- http://127.0.0.1:8080
#[tokio::main]
async fn main() -> Result<()> {
    pretty_env_logger::init();

    let args: Vec<String> = env::args().collect();

    // Check if we should run the server
    if args.len() > 1 && args[1] == "--server" {
        let port = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(8080);
        return simple_100_continue_server(port).await;
    }

    // Some simple CLI args requirements...
    let url = match args.get(1) {
        Some(url) => url,
        None => {
            println!("Usage: client <url>");
            println!("       client --server [port]");
            return Ok(());
        }
    };

    // HTTPS requires picking a TLS implementation, so give a better
    // warning if the user tries to request an 'https' URL.
    let url = url.parse::<hyper::Uri>().unwrap();
    if url.scheme_str() != Some("http") {
        println!("This example only works with 'http' URLs.");
        return Ok(());
    }

    post(url).await
}

async fn post(url: hyper::Uri) -> Result<()> {
    let host = url.host().expect("uri has no host");
    let port = url.port_u16().unwrap_or(80);
    let addr = format!("{}:{}", host, port);
    let stream = TcpStream::connect(addr).await?;
    let io = TokioIo::new(stream);

    let (mut sender, conn) = hyper::client::conn::http1::handshake(io).await?;
    tokio::task::spawn(async move {
        if let Err(err) = conn.await {
            println!("Connection failed: {:?}", err);
        }
    });

    let authority = url.authority().unwrap().clone();
    let path = url.path();

    // Send a request with a fixed length body and an Expect: 100-continue header.
    // The body will not start sending until we receive a signal on the oneshot channel.
    let body = Full::new(Bytes::from("hello"));
    let (tx, rx) = oneshot::channel();
    let delayed_body = DelayedBody {
        inner: body,
        rx: Some(rx),
    };

    let mut req = Request::builder()
        .method("POST")
        .uri(path)
        .header(hyper::header::HOST, authority.as_str())
        .header(hyper::header::CONTENT_LENGTH, "5")
        .header(hyper::header::EXPECT, "100-continue")
        .body(delayed_body)?;

    let tx = std::sync::Arc::new(std::sync::Mutex::new(Some(tx)));

    // Register a callback for informational responses (100 Continue)
    // that will send a signal to the body to start processing.
    on_informational(&mut req, move |res| {
        if res.status() == 100 {
            println!("Received 100 Continue from server.");
            // We got 100 continue from the server
            let mut tx = tx.lock().unwrap();
            if let Some(tx) = tx.take() {
                let _ = tx.send(());
            }
        }
    });

    let mut res = sender.send_request(req).await?;

    println!("Response: {}", res.status());
    println!("Headers: {:#?}\n", res.headers());

    // Stream the body, writing each chunk to stdout as we get it
    // (instead of buffering and printing at the end).
    while let Some(next) = res.frame().await {
        let frame = next?;
        if let Some(chunk) = frame.data_ref() {
            stdout().write_all(chunk).await?;
        }
    }

    println!("\n\nDone!");

    Ok(())
}

/// A simple TCP server that handles 100 Continue requests
async fn simple_100_continue_server(port: u16) -> Result<()> {
    let listener = TcpListener::bind(format!("127.0.0.1:{}", port)).await?;
    println!("Server listening on 127.0.0.1:{}", port);

    loop {
        let (mut stream, _) = listener.accept().await?;

        tokio::spawn(async move {
            let mut buffer = vec![0; 1024];
            let mut request = String::new();

            // Read the HTTP request
            loop {
                match stream.read(&mut buffer).await {
                    Ok(0) => break, // Connection closed
                    Ok(n) => {
                        request.push_str(&String::from_utf8_lossy(&buffer[..n]));
                        if request.contains("\r\n\r\n") {
                            break;
                        }
                    }
                    Err(e) => {
                        println!("Error reading from stream: {}", e);
                        return;
                    }
                }
            }

            println!("Received request:\n{}", request);

            // Check if request has Expect: 100-continue header
            if request.contains("expect: 100-continue") {
                println!("Sending 100 Continue response");

                // Send 100 Continue response
                let continue_response = "HTTP/1.1 100 Continue\r\n\r\n";
                if let Err(e) = stream.write_all(continue_response.as_bytes()).await {
                    println!("Error sending 100 Continue: {}", e);
                    return;
                }

                // Read the request body
                let mut body = String::new();
                let mut buffer = vec![0; 1024];

                match stream.read(&mut buffer).await {
                    Ok(n) => {
                        body.push_str(&String::from_utf8_lossy(&buffer[..n]));
                        println!("Received body: {}", body);
                    }
                    Err(e) => {
                        println!("Error reading body: {}", e);
                        return;
                    }
                }
            }

            // Send final response
            let response = "HTTP/1.1 200 OK\r\n\
                Content-Length: 13\r\n\
                \r\n\
                Hello, World!";
            if let Err(e) = stream.write_all(response.as_bytes()).await {
                println!("Error sending response: {}", e);
            }
        });
    }
}
