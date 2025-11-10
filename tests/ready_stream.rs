use http_body_util::StreamBody;
use hyper::body::Bytes;
use hyper::body::Frame;
use hyper::rt::{Read, ReadBufCursor, Write};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Response, StatusCode};
use pin_project_lite::pin_project;
use std::convert::Infallible;
use std::io;
use std::pin::Pin;
use std::task::{ready, Context, Poll};
use tokio::sync::mpsc;

pin_project! {
    #[derive(Debug)]
    pub struct TxReadyStream {
        #[pin]
        read_rx: mpsc::UnboundedReceiver<Vec<u8>>,
        write_tx: mpsc::UnboundedSender<Vec<u8>>,
        read_buffer: Vec<u8>,
        poll_since_write:bool,
        flush_count: usize,
        panic_task: Option<tokio::task::JoinHandle<()>>,
    }
}

impl TxReadyStream {
    fn new(
        read_rx: mpsc::UnboundedReceiver<Vec<u8>>,
        write_tx: mpsc::UnboundedSender<Vec<u8>>,
    ) -> Self {
        Self {
            read_rx,
            write_tx,
            read_buffer: Vec::new(),
            poll_since_write: true,
            flush_count: 0,
            panic_task: None,
        }
    }

    /// Create a new pair of connected ReadyStreams. Returns two streams that are connected to each other.
    fn new_pair() -> (Self, Self) {
        let (s1_tx, s2_rx) = mpsc::unbounded_channel();
        let (s2_tx, s1_rx) = mpsc::unbounded_channel();
        let s1 = Self::new(s1_rx, s1_tx);
        let s2 = Self::new(s2_rx, s2_tx);
        (s1, s2)
    }

    /// Send data to the other end of the stream (this will be available for reading on the other stream)
    fn send(&self, data: &[u8]) -> Result<(), mpsc::error::SendError<Vec<u8>>> {
        self.write_tx.send(data.to_vec())
    }

    /// Receive data written to this stream by the other end (async)
    async fn recv(&mut self) -> Option<Vec<u8>> {
        self.read_rx.recv().await
    }
}

impl Read for TxReadyStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        mut buf: ReadBufCursor<'_>,
    ) -> Poll<io::Result<()>> {
        let mut this = self.as_mut().project();

        // First, try to satisfy the read request from the internal buffer
        if !this.read_buffer.is_empty() {
            let to_read = std::cmp::min(this.read_buffer.len(), buf.remaining());
            // Copy data from internal buffer to the read buffer
            buf.put_slice(&this.read_buffer[..to_read]);
            // Remove the consumed data from the internal buffer
            this.read_buffer.drain(..to_read);
            return Poll::Ready(Ok(()));
        }

        // If internal buffer is empty, try to get data from the channel
        match this.read_rx.try_recv() {
            Ok(data) => {
                // Copy as much data as we can fit in the buffer
                let to_read = std::cmp::min(data.len(), buf.remaining());
                buf.put_slice(&data[..to_read]);

                // Store any remaining data in the internal buffer for next time
                if to_read < data.len() {
                    let remaining = &data[to_read..];
                    this.read_buffer.extend_from_slice(remaining);
                }
                Poll::Ready(Ok(()))
            }
            Err(mpsc::error::TryRecvError::Empty) => {
                match ready!(this.read_rx.poll_recv(cx)) {
                    Some(data) => {
                        // Copy as much data as we can fit in the buffer
                        let to_read = std::cmp::min(data.len(), buf.remaining());
                        buf.put_slice(&data[..to_read]);

                        // Store any remaining data in the internal buffer for next time
                        if to_read < data.len() {
                            let remaining = &data[to_read..];
                            this.read_buffer.extend_from_slice(remaining);
                        }
                        Poll::Ready(Ok(()))
                    }
                    None => Poll::Ready(Ok(())),
                }
            }
            Err(mpsc::error::TryRecvError::Disconnected) => {
                // Channel closed, return EOF
                Poll::Ready(Ok(()))
            }
        }
    }
}

impl Write for TxReadyStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        if !self.poll_since_write {
            return Poll::Pending;
        }
        self.poll_since_write = false;
        let this = self.project();
        let buf = Vec::from(&buf[..buf.len()]);
        let len = buf.len();

        // Send data through the channel - this should always be ready for unbounded channels
        match this.write_tx.send(buf) {
            Ok(_) => {
                // Increment write count
                Poll::Ready(Ok(len))
            }
            Err(_) => {
                println!("ReadyStream::poll_write failed - channel closed");
                Poll::Ready(Err(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "Write channel closed",
                )))
            }
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.flush_count += 1;
        // We require two flushes to complete each chunk, simulating a success at the end of the old
        // poll loop. After all chunks are written, we always succeed on flush to allow for finish.
        if self.flush_count % 2 != 0 && self.flush_count < TOTAL_CHUNKS * 2 {
            // Spawn panic task if not already spawned
            if self.panic_task.is_none() {
                let task = tokio::spawn(async {
                    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                });
                self.panic_task = Some(task);
            }
            return Poll::Pending;
        }

        // Abort the panic task if it exists
        if let Some(task) = self.panic_task.take() {
            println!("Task polled to completion. Aborting panic (aka waker stand-in task).");
            task.abort();
        }

        self.poll_since_write = true;
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

const TOTAL_CHUNKS: usize = 16;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn body_test() {
    // Create a pair of connected streams
    let (server_stream, mut client_stream) = TxReadyStream::new_pair();

    let mut http_builder = http1::Builder::new();
    http_builder.max_buf_size(CHUNK_SIZE);
    const CHUNK_SIZE: usize = 64 * 1024;
    let service = service_fn(|_| async move {
        println!(
            "Creating payload of {} chunks of {} KiB each ({} MiB total)...",
            TOTAL_CHUNKS,
            CHUNK_SIZE / 1024,
            TOTAL_CHUNKS * CHUNK_SIZE / (1024 * 1024)
        );
        let bytes = Bytes::from(vec![0; CHUNK_SIZE]);
        let data = vec![bytes.clone(); TOTAL_CHUNKS];
        let stream = futures_util::stream::iter(
            data.into_iter()
                .map(|b| Ok::<_, Infallible>(Frame::data(b))),
        );
        let body = StreamBody::new(stream);
        println!("Server: Sending data response...");
        Ok::<_, hyper::Error>(
            Response::builder()
                .status(StatusCode::OK)
                .header("content-type", "application/octet-stream")
                .header("content-length", (TOTAL_CHUNKS * CHUNK_SIZE).to_string())
                .body(body)
                .unwrap(),
        )
    });

    let server_task = tokio::spawn(async move {
        let conn = http_builder.serve_connection(server_stream, service);
        if let Err(e) = conn.await {
            println!("Server connection error: {}", e);
        }
    });

    let get_request = "GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n";
    client_stream.send(get_request.as_bytes()).unwrap();

    println!("Client is reading response...");
    let mut bytes_received = 0;
    while let Some(chunk) = client_stream.recv().await {
        bytes_received += chunk.len();
    }
    // Clean up
    server_task.abort();

    println!("Client done receiving bytes: {}", bytes_received);
}
