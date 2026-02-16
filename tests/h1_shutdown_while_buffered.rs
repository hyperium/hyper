// Test: Ensures poll_shutdown() is never called with buffered data
//
// Reproduces rare timing bug where HTTP/1.1 server calls shutdown() on a socket while response
// data is still buffered (not flushed), leading to data loss.
//
// Scenario:
// 1. Request fully received and read.
// 2. Server computes a "large" response with Full::new()
// 3. Socket accepts only a chunk of response and then pends
// 3. Flush returns Pending (remaining data still buffered), result ignored
// 4. self.conn.wants_read_again() is false and poll_loop returns Ready
// 5. BUG: poll_shutdown called prematurely and buffered body is lost
// 6. FIX: poll_loop checks flush result and returns Pending, giving the chance for poll_loop to
//    run again

use std::{
    pin::Pin,
    sync::{Arc, Mutex},
    task::Poll,
    time::Duration,
};

use bytes::Bytes;
use http::{Request, Response};
use http_body_util::Full;
use hyper::{body::Incoming, service::service_fn};
use support::TokioIo;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::{TcpListener, TcpStream},
    time::{sleep, timeout},
};
mod support;

#[derive(Debug, Default)]
struct PendingStreamStatistics {
    bytes_written: usize,
    total_attempted: usize,
    shutdown_called_with_buffered: bool,
    buffered_at_shutdown: usize,
}

// Simple struct that simply does one write and then pends perpetually
struct PendingStream {
    inner: TcpStream,
    // Keep track of how many times we entered poll_write so as to be able to write only the first
    // time out
    write_count: usize,
    // Only write this chunk size out of full buffer
    write_chunk_size: usize,
    stats: Arc<Mutex<PendingStreamStatistics>>,
}

impl PendingStream {
    fn new(
        inner: TcpStream,
        write_chunk_size: usize,
        stats: Arc<Mutex<PendingStreamStatistics>>,
    ) -> Self {
        Self {
            inner,
            stats,
            write_chunk_size,
            write_count: 0,
        }
    }
}

impl AsyncRead for PendingStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

impl AsyncWrite for PendingStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        self.write_count += 1;

        let mut stats = self.stats.lock().unwrap();
        stats.total_attempted += buf.len();

        if self.write_count == 1 {
            // First write: partial only
            let partial = std::cmp::min(buf.len(), self.write_chunk_size);
            drop(stats);

            let result = Pin::new(&mut self.inner).poll_write(cx, &buf[..partial]);
            if let Poll::Ready(Ok(n)) = result {
                self.stats.lock().unwrap().bytes_written += n;
            }
            return result;
        }

        // Block all further writes to simulate pending buffer
        Poll::Pending
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<std::io::Result<()>> {
        let mut stats = self.stats.lock().unwrap();
        let buffered = stats.total_attempted - stats.bytes_written;

        if buffered > 0 {
            eprintln!(
                "\n❌BUG: shutdown() called with {} bytes buffered",
                buffered
            );
            stats.shutdown_called_with_buffered = true;
            stats.buffered_at_shutdown = buffered;
        }
        drop(stats);
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<std::io::Result<()>> {
        let stats = self.stats.lock().unwrap();
        let buffered = stats.total_attempted - stats.bytes_written;

        if buffered > 0 {
            return Poll::Pending;
        }

        drop(stats);
        Pin::new(&mut self.inner).poll_flush(cx)
    }
}

// Test doesn't necessarily check that the connections ended successfully but mainly that shutdown
// wasn't called with data still remaining within hyper's internal buffer
#[tokio::test]
async fn test_no_premature_shutdown_while_buffered() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let stats = Arc::new(Mutex::new(PendingStreamStatistics::default()));

    let stats_clone = stats.clone();
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let pending_stream = PendingStream::new(stream, 212_992, stats_clone);
        let io = TokioIo::new(pending_stream);

        let service = service_fn(|_req: Request<Incoming>| async move {
            // Larger Full response than write_chunk_size
            let body = Full::new(Bytes::from(vec![b'X'; 500_000]));
            Ok::<_, hyper::Error>(Response::new(body))
        });

        hyper::server::conn::http1::Builder::new()
            .serve_connection(io, service)
            .await
    });

    // Wait for server to be ready
    sleep(Duration::from_millis(50)).await;

    // Client sends request
    tokio::spawn(async move {
        let mut stream = TcpStream::connect(addr).await.unwrap();

        use tokio::io::AsyncWriteExt;

        stream
            .write_all(
                b"POST / HTTP/1.1\r\n\
            Host: localhost\r\n\
            Transfer-Encoding: chunked\r\n\
            \r\n",
            )
            .await
            .unwrap();

        stream.write_all(b"A\r\nHello World\r\n").await.unwrap();
        stream.write_all(b"0\r\n\r\n").await.unwrap();
        stream.flush().await.unwrap();

        // keep connection open
        sleep(Duration::from_secs(2)).await;
    });

    // Wait for completion
    let result = timeout(Duration::from_millis(900), server).await;

    let stats = stats.lock().unwrap();

    assert!(
        !stats.shutdown_called_with_buffered,
        "shutdown() called with {} bytes still buffered (wrote {} of {} bytes)",
        stats.buffered_at_shutdown, stats.bytes_written, stats.total_attempted
    );
    if let Ok(Ok(conn_result)) = result {
        conn_result.ok();
    }
}
