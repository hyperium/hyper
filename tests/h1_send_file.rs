#![cfg(unix)]
// Tests for `hyper::ext::SendFile`: an HTTP/1 response body written by the
// IO's `poll_write_file` rather than polled from the body.
//
// The IO here implements `poll_write_file` by reading a few bytes of the file
// at the offset and writing them to the socket, and it returns `Pending` before
// every such write, so a body always takes several calls across several polls
// and exercises the resumption bookkeeping. The fallback body is given
// different bytes from the file, so a response shows which one was sent.

use std::convert::Infallible;
use std::fs::File;
use std::io;
use std::os::unix::fs::FileExt;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use bytes::Bytes;
use futures_util::stream;
use http_body_util::{Full, StreamBody};
use hyper::body::Frame;
use hyper::ext::SendFile;
use hyper::rt::{Read, ReadBufCursor, Write};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::Response;
use support::TokioIo;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::timeout;

mod support;

const FILE: &[u8] = b"hello from the file";
const FALLBACK: &[u8] = b"a fallback body....";

/// How many bytes one `poll_write_file` call sends at most.
const STEP: usize = 4;

struct FileIo {
    inner: TokioIo<TcpStream>,
    supports: bool,
    file_writes: Arc<AtomicUsize>,
    ready: bool,
}

impl Read for FileIo {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: ReadBufCursor<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

impl Write for FileIo {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.inner).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }

    fn supports_write_file(&self) -> bool {
        self.supports
    }

    fn poll_write_file(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        file: &File,
        offset: u64,
        len: usize,
    ) -> Poll<io::Result<usize>> {
        assert!(self.supports, "poll_write_file called on an IO without it");
        self.ready = !self.ready;
        if !self.ready {
            cx.waker().wake_by_ref();
            return Poll::Pending;
        }
        let mut buf = [0u8; STEP];
        let n = file.read_at(&mut buf[..len.min(STEP)], offset)?;
        if n == 0 {
            return Poll::Ready(Ok(0));
        }
        let written = Pin::new(&mut self.inner).poll_write(cx, &buf[..n]);
        if let Poll::Ready(Ok(_)) = written {
            self.file_writes.fetch_add(1, Ordering::SeqCst);
        }
        written
    }
}

/// A file with `FILE` in it, removed on drop.
struct TempFile(PathBuf);

impl TempFile {
    fn new(name: &str) -> (TempFile, Arc<File>) {
        let path =
            std::env::temp_dir().join(format!("hyper-send-file-{}-{}", std::process::id(), name));
        std::fs::write(&path, FILE).unwrap();
        let file = Arc::new(File::open(&path).unwrap());
        (TempFile(path), file)
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Serve one connection with `respond`, returning the client socket, the
/// count of `poll_write_file` calls, and the connection's result.
async fn serve<F, B>(
    supports: bool,
    respond: F,
) -> (
    TcpStream,
    Arc<AtomicUsize>,
    tokio::task::JoinHandle<hyper::Result<()>>,
)
where
    F: Fn() -> Response<B> + Send + Sync + 'static,
    B: hyper::body::Body + Send + 'static,
    B::Data: Send,
    B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    serve_with(http1::Builder::new(), supports, respond).await
}

async fn serve_with<F, B>(
    builder: http1::Builder,
    supports: bool,
    respond: F,
) -> (
    TcpStream,
    Arc<AtomicUsize>,
    tokio::task::JoinHandle<hyper::Result<()>>,
)
where
    F: Fn() -> Response<B> + Send + Sync + 'static,
    B: hyper::body::Body + Send + 'static,
    B::Data: Send,
    B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let file_writes = Arc::new(AtomicUsize::new(0));

    let counter = file_writes.clone();
    let respond = Arc::new(respond);
    let server = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        let io = FileIo {
            inner: TokioIo::new(socket),
            supports,
            file_writes: counter,
            ready: false,
        };
        let service = service_fn(move |_req| {
            let respond = respond.clone();
            async move { Ok::<_, Infallible>(respond()) }
        });
        builder.serve_connection(io, service).await
    });

    let client = TcpStream::connect(addr).await.unwrap();
    (client, file_writes, server)
}

/// Read one response: its head, and a body of `Content-Length` bytes.
async fn read_response(client: &mut TcpStream) -> (String, Vec<u8>) {
    let (head, body, rest) = read_responses(client, Vec::new()).await;
    assert!(
        rest.is_empty(),
        "extra bytes: {:?}",
        String::from_utf8_lossy(&rest)
    );
    (head, body)
}

/// Read one response after the bytes in `received`, returning whatever of
/// the next response came with it.
async fn read_responses(
    client: &mut TcpStream,
    mut received: Vec<u8>,
) -> (String, Vec<u8>, Vec<u8>) {
    let mut buf = [0u8; 256];
    let read = timeout(Duration::from_secs(5), async {
        loop {
            if let Some(end) = find(&received, b"\r\n\r\n") {
                let head = String::from_utf8(received[..end].to_vec()).unwrap();
                let len = content_length(&head);
                if received.len() >= end + 4 + len {
                    let body = received[end + 4..end + 4 + len].to_vec();
                    let rest = received.split_off(end + 4 + len);
                    return (head, body, rest);
                }
            }
            let n = client.read(&mut buf).await.unwrap();
            assert!(
                n != 0,
                "connection closed mid-response: {:?}",
                String::from_utf8_lossy(&received)
            );
            received.extend_from_slice(&buf[..n]);
        }
    })
    .await;
    read.expect("response never completed")
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn content_length(head: &str) -> usize {
    head.lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            if name.eq_ignore_ascii_case("content-length") {
                value.trim().parse().ok()
            } else {
                None
            }
        })
        .unwrap_or(0)
}

fn with_file(
    file: &Arc<File>,
    offset: u64,
    len: u64,
    body: &'static [u8],
) -> Response<Full<Bytes>> {
    let mut response = Response::new(Full::new(Bytes::from_static(body)));
    response
        .extensions_mut()
        .insert(SendFile::new(file.clone(), offset, len));
    response
}

const GET: &[u8] = b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n";

#[tokio::test]
async fn sends_the_file_instead_of_the_body() {
    let (_tmp, file) = TempFile::new("instead");
    let (mut client, file_writes, _server) = serve(true, move || {
        with_file(&file, 0, FILE.len() as u64, FALLBACK)
    })
    .await;

    client.write_all(GET).await.unwrap();
    let (head, body) = read_response(&mut client).await;

    assert!(head.starts_with("HTTP/1.1 200 OK"), "{head}");
    assert_eq!(content_length(&head), FILE.len());
    assert_eq!(body, FILE);
    assert_eq!(
        file_writes.load(Ordering::SeqCst),
        (FILE.len() + STEP - 1) / STEP
    );
}

#[tokio::test]
async fn sends_a_range_of_the_file() {
    let (_tmp, file) = TempFile::new("range");
    let (mut client, _, _server) = serve(true, move || with_file(&file, 6, 4, b"xxxx")).await;

    client.write_all(GET).await.unwrap();
    let (_, body) = read_response(&mut client).await;

    assert_eq!(body, b"from");
}

#[tokio::test]
async fn keeps_the_connection_alive_after_a_file() {
    let (_tmp, file) = TempFile::new("keep-alive");
    let (mut client, file_writes, _server) = serve(true, move || {
        with_file(&file, 0, FILE.len() as u64, FALLBACK)
    })
    .await;

    for _ in 0..3 {
        client.write_all(GET).await.unwrap();
        let (_, body) = read_response(&mut client).await;
        assert_eq!(body, FILE);
    }
    assert_eq!(
        file_writes.load(Ordering::SeqCst),
        3 * ((FILE.len() + STEP - 1) / STEP)
    );
}

#[tokio::test]
async fn finishes_the_file_after_the_client_stops_sending() {
    let (_tmp, file) = TempFile::new("half-close");
    // Without half-close, hyper drops a connection whose peer stops sending.
    let mut builder = http1::Builder::new();
    builder.half_close(true);
    let (mut client, _, _server) = serve_with(builder, true, move || {
        with_file(&file, 0, FILE.len() as u64, FALLBACK)
    })
    .await;

    client.write_all(GET).await.unwrap();
    client.shutdown().await.unwrap();
    let (_, body) = read_response(&mut client).await;
    assert_eq!(body, FILE);
}

#[tokio::test]
async fn finishes_the_file_before_closing_a_connection_marked_close() {
    let (_tmp, file) = TempFile::new("close");
    let (mut client, _, _server) = serve(true, move || {
        with_file(&file, 0, FILE.len() as u64, FALLBACK)
    })
    .await;

    client
        .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .await
        .unwrap();
    let mut received = Vec::new();
    timeout(Duration::from_secs(5), client.read_to_end(&mut received))
        .await
        .expect("connection never closed")
        .unwrap();
    assert!(
        received.ends_with(FILE),
        "{:?}",
        String::from_utf8_lossy(&received)
    );
}

#[tokio::test]
async fn sends_pipelined_responses_in_order() {
    let (_tmp, file) = TempFile::new("pipelined");
    let (mut client, _, _server) = serve(true, move || {
        with_file(&file, 0, FILE.len() as u64, FALLBACK)
    })
    .await;

    client.write_all(&[GET, GET].concat()).await.unwrap();
    let (_, first, rest) = read_responses(&mut client, Vec::new()).await;
    let (_, second, rest) = read_responses(&mut client, rest).await;
    assert_eq!(first, FILE);
    assert_eq!(second, FILE);
    assert!(rest.is_empty());
}

#[tokio::test]
async fn flushes_the_head_before_the_file_when_pipeline_flush_is_on() {
    let (_tmp, file) = TempFile::new("pipeline-flush");
    let mut builder = http1::Builder::new();
    builder.pipeline_flush(true);
    let (mut client, _, _server) = serve_with(builder, true, move || {
        with_file(&file, 0, FILE.len() as u64, FALLBACK)
    })
    .await;

    client.write_all(&[GET, GET, GET].concat()).await.unwrap();
    let mut rest = Vec::new();
    for _ in 0..3 {
        let (head, body, more) = read_responses(&mut client, rest).await;
        assert!(head.starts_with("HTTP/1.1 200 OK"), "{head}");
        assert_eq!(body, FILE);
        rest = more;
    }
    assert!(rest.is_empty());
}

#[tokio::test]
async fn falls_back_to_the_body_when_the_io_cannot_write_files() {
    let (_tmp, file) = TempFile::new("unsupported");
    let (mut client, file_writes, _server) = serve(false, move || {
        with_file(&file, 0, FILE.len() as u64, FALLBACK)
    })
    .await;

    client.write_all(GET).await.unwrap();
    let (_, body) = read_response(&mut client).await;

    assert_eq!(body, FALLBACK);
    assert_eq!(file_writes.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn falls_back_to_the_body_when_the_length_differs() {
    let (_tmp, file) = TempFile::new("length");
    let (mut client, file_writes, _server) =
        serve(true, move || with_file(&file, 0, 4, FALLBACK)).await;

    client.write_all(GET).await.unwrap();
    let (_, body) = read_response(&mut client).await;

    assert_eq!(body, FALLBACK);
    assert_eq!(file_writes.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn falls_back_to_a_chunked_body() {
    let (_tmp, file) = TempFile::new("chunked");
    let (mut client, file_writes, _server) = serve(true, move || {
        let chunks = stream::iter(vec![Ok::<_, Infallible>(Frame::data(Bytes::from_static(
            FALLBACK,
        )))]);
        let mut response = Response::new(StreamBody::new(chunks));
        response
            .extensions_mut()
            .insert(SendFile::new(file.clone(), 0, FILE.len() as u64));
        response
    })
    .await;

    client.write_all(GET).await.unwrap();
    let mut received = Vec::new();
    timeout(Duration::from_secs(5), async {
        let mut buf = [0u8; 256];
        while !received.ends_with(b"0\r\n\r\n") {
            let n = client.read(&mut buf).await.unwrap();
            assert!(n != 0, "connection closed mid-response");
            received.extend_from_slice(&buf[..n]);
        }
    })
    .await
    .expect("response never completed");

    let received = String::from_utf8(received).unwrap();
    assert!(
        received.contains("transfer-encoding: chunked"),
        "{received}"
    );
    assert!(received.contains("a fallback body"), "{received}");
    assert_eq!(file_writes.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn sends_no_body_for_head() {
    let (_tmp, file) = TempFile::new("head");
    let (mut client, file_writes, _server) = serve(true, move || {
        with_file(&file, 0, FILE.len() as u64, FALLBACK)
    })
    .await;

    client
        .write_all(b"HEAD / HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .await
        .unwrap();
    // A HEAD response announces the length but carries no body, so read the
    // head alone and then prove nothing follows it.
    let mut received = Vec::new();
    let mut buf = [0u8; 256];
    timeout(Duration::from_secs(5), async {
        while find(&received, b"\r\n\r\n").is_none() {
            let n = client.read(&mut buf).await.unwrap();
            assert!(n != 0, "connection closed mid-response");
            received.extend_from_slice(&buf[..n]);
        }
    })
    .await
    .expect("response never completed");
    assert!(
        received.ends_with(b"\r\n\r\n"),
        "body after a HEAD response"
    );
    client.write_all(GET).await.unwrap();
    let (_, body) = read_response(&mut client).await;
    assert_eq!(body, FILE);

    assert_eq!(
        file_writes.load(Ordering::SeqCst),
        (FILE.len() + STEP - 1) / STEP
    );
}

#[tokio::test]
async fn errors_when_the_file_is_shorter_than_its_length() {
    let (_tmp, file) = TempFile::new("short");
    // Asks for more of the file than there is, from a body that agrees.
    const LONG: &[u8] = b"a fallback body longer than the file";
    let (mut client, _, server) =
        serve(true, move || with_file(&file, 0, LONG.len() as u64, LONG)).await;

    client.write_all(GET).await.unwrap();
    let result = timeout(Duration::from_secs(5), server)
        .await
        .expect("connection never finished")
        .unwrap();
    result.expect_err("a short file must fail the connection");

    // The client gets the file's bytes, and then the connection closes short.
    let mut received = Vec::new();
    client.read_to_end(&mut received).await.unwrap();
    assert!(
        received.ends_with(FILE),
        "{:?}",
        String::from_utf8_lossy(&received)
    );
}
