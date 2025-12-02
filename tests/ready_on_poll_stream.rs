#[path = "h1_server/mod.rs"]
mod h1_server;

use h1_server::{fixture, init_tracing, StreamReadHalf};
use hyper::rt::{Read, ReadBufCursor, Write};
use pin_project_lite::pin_project;
use std::future::Future;
use std::io;
use std::pin::Pin;
use std::task::{ready, Context, Poll};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::Sleep;
use tracing::error;

pin_project! {
    #[derive(Debug)]
    pub struct ReadyOnPollStream {
        #[pin]
        read_half: StreamReadHalf,
        write_tx: mpsc::UnboundedSender<Vec<u8>>,
        #[pin]
        pending_write: Option<Pin<Box<Sleep>>>,
        poll_since_write: bool,
        flush_count: usize,
    }
}

impl ReadyOnPollStream {
    fn new(
        read_rx: mpsc::UnboundedReceiver<Vec<u8>>,
        write_tx: mpsc::UnboundedSender<Vec<u8>>,
    ) -> Self {
        Self {
            read_half: StreamReadHalf::new(read_rx),
            write_tx,
            poll_since_write: true,
            flush_count: 0,
            pending_write: None,
        }
    }

    /// Create a new server stream and client pair.
    /// Returns a server stream (Read+Write) and a client (rx/tx channels).
    pub fn new_pair() -> (Self, fixture::Client) {
        let (client_tx, server_rx) = mpsc::unbounded_channel();
        let (server_tx, client_rx) = mpsc::unbounded_channel();
        let server = Self::new(server_rx, server_tx);
        let client = fixture::Client {
            rx: client_rx,
            tx: client_tx,
        };
        (server, client)
    }
}

impl Read for ReadyOnPollStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: ReadBufCursor<'_>,
    ) -> Poll<io::Result<()>> {
        self.as_mut().project().read_half.poll_read(cx, buf)
    }
}

const WRITE_DELAY: Duration = Duration::from_millis(100);

impl Write for ReadyOnPollStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        if let Some(sleep) = self.pending_write.as_mut() {
            let sleep = sleep.as_mut();
            ready!(Future::poll(sleep, cx));
        }
        {
            let mut this = self.as_mut().project();
            this.pending_write
                .set(Some(Box::pin(tokio::time::sleep(WRITE_DELAY))));
        }
        let Some(sleep) = self.pending_write.as_mut() else {
            panic!("Sleep should have just been set");
        };
        // poll the future so that we can woken
        let sleep = sleep.as_mut();
        let Poll::Pending = Future::poll(sleep, cx) else {
            panic!("Sleep always be pending on first poll")
        };

        let this = self.project();
        let buf = Vec::from(&buf[..buf.len()]);
        let len = buf.len();

        // Send data through the channel - this should always be ready for unbounded channels
        match this.write_tx.send(buf) {
            Ok(_) => Poll::Ready(Ok(len)),
            Err(_) => {
                error!("ReadyStream::poll_write failed - channel closed");
                Poll::Ready(Err(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "Write channel closed",
                )))
            }
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.flush_count += 1;
        // We require two flushes to complete each chunk, simulating a success at the end of the old
        // poll loop. After all chunks are written, we always succeed on flush to allow for finish.
        const TOTAL_CHUNKS: usize = 16;
        if self.flush_count % 2 != 0 && self.flush_count < TOTAL_CHUNKS * 2 {
            if let Some(sleep) = self.pending_write.as_mut() {
                let sleep = sleep.as_mut();
                ready!(Future::poll(sleep, cx));
            } else {
                return Poll::Pending;
            }
        }
        let mut this = self.as_mut().project();
        this.pending_write.set(None);
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn body_test() {
    init_tracing();
    let (server, client) = ReadyOnPollStream::new_pair();
    let config = fixture::TestConfig::with_timeout(WRITE_DELAY * 2);
    fixture::run(server, client, config).await;
}
