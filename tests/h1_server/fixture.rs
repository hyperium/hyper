use http_body_util::StreamBody;
use hyper::body::Bytes;
use hyper::body::Frame;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Response, StatusCode};
use std::convert::Infallible;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::timeout;
use tracing::{error, info};

pub struct TestConfig {
    pub total_chunks: usize,
    pub chunk_size: usize,
    pub chunk_timeout: Duration,
}

impl TestConfig {
    pub fn with_timeout(chunk_timeout: Duration) -> Self {
        Self {
            total_chunks: 16,
            chunk_size: 64 * 1024,
            chunk_timeout,
        }
    }
}

pub struct Client {
    pub rx: mpsc::UnboundedReceiver<Vec<u8>>,
    pub tx: mpsc::UnboundedSender<Vec<u8>>,
}

pub async fn run<S>(server: S, mut client: Client, config: TestConfig)
where
    S: hyper::rt::Read + hyper::rt::Write + Send + Unpin + 'static,
{
    let mut http_builder = http1::Builder::new();
    http_builder.max_buf_size(config.chunk_size);

    let total_chunks = config.total_chunks;
    let chunk_size = config.chunk_size;

    let service = service_fn(move |_| {
        let total_chunks = total_chunks;
        let chunk_size = chunk_size;
        async move {
            info!(
                "Creating payload of {} chunks of {} KiB each ({} MiB total)...",
                total_chunks,
                chunk_size / 1024,
                total_chunks * chunk_size / (1024 * 1024)
            );
            let bytes = Bytes::from(vec![0; chunk_size]);
            let data = vec![bytes.clone(); total_chunks];
            let stream = futures_util::stream::iter(
                data.into_iter()
                    .map(|b| Ok::<_, Infallible>(Frame::data(b))),
            );
            let body = StreamBody::new(stream);
            info!("Server: Sending data response...");
            Ok::<_, hyper::Error>(
                Response::builder()
                    .status(StatusCode::OK)
                    .header("content-type", "application/octet-stream")
                    .header("content-length", (total_chunks * chunk_size).to_string())
                    .body(body)
                    .unwrap(),
            )
        }
    });

    let server_task = tokio::spawn(async move {
        let conn = http_builder.serve_connection(Box::pin(server), service);
        let conn_result = conn.await;
        if let Err(e) = &conn_result {
            error!("Server connection error: {}", e);
        }
        conn_result
    });

    let get_request = "GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n";
    client
        .tx
        .send(get_request.as_bytes().to_vec())
        .map_err(|e| {
            Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to send request: {}", e),
            ))
        })
        .unwrap();

    info!("Client is reading response...");
    let mut bytes_received = 0;
    let mut all_data = Vec::new();
    loop {
        match timeout(config.chunk_timeout, client.rx.recv()).await {
            Ok(Some(chunk)) => {
                bytes_received += chunk.len();
                all_data.extend_from_slice(&chunk);
            }
            Ok(None) => break,
            Err(_) => {
                panic!(
                    "Chunk timeout: chunk took longer than {:?}",
                    config.chunk_timeout
                );
            }
        }
    }

    // Clean up
    let result = server_task.await.unwrap();
    result.unwrap();

    // Parse HTTP response to find body start
    // HTTP response format: "HTTP/1.1 200 OK\r\n...headers...\r\n\r\n<body>"
    let body_start = all_data
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|pos| pos + 4)
        .unwrap_or(0);

    let body_bytes = bytes_received - body_start;
    assert_eq!(
        body_bytes,
        config.total_chunks * config.chunk_size,
        "Expected {} body bytes, got {} (total received: {}, headers: {})",
        config.total_chunks * config.chunk_size,
        body_bytes,
        bytes_received,
        body_start
    );
    info!(bytes_received, body_bytes, "Client done receiving bytes");
}
