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
    pub struct UnbufferedStream {
        #[pin]
        read_half: StreamReadHalf,
        #[pin]
        pending_write: Option<Pin<Box<Sleep>>>,
        write_tx: mpsc::UnboundedSender<Vec<u8>>,
        poll_cnt: usize,
    }
}

impl UnbufferedStream {
    fn new(
        read_rx: mpsc::UnboundedReceiver<Vec<u8>>,
        write_tx: mpsc::UnboundedSender<Vec<u8>>,
    ) -> Self {
        Self {
            read_half: StreamReadHalf::new(read_rx),
            write_tx,
            pending_write: None,
            poll_cnt: 0,
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

const WRITE_DELAY: Duration = Duration::from_millis(100);

impl Read for UnbufferedStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: ReadBufCursor<'_>,
    ) -> Poll<io::Result<()>> {
        let response = self.as_mut().project().read_half.poll_read(cx, buf);
        response
    }
}

impl Write for UnbufferedStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.poll_cnt += 1;
        let poll_cnt = self.poll_cnt;
        if let Some(sleep) = self.pending_write.as_mut() {
            let sleep = sleep.as_mut();
            if poll_cnt > 4 {
                return Poll::Ready(Err(io::Error::other("We are being hot polled!")));
            }
            ready!(Future::poll(sleep, cx));
            let mut this = self.as_mut().project();
            this.pending_write.set(None);
            *this.poll_cnt = 0;
        }
        let len = buf.len();
        {
            let mut this = self.as_mut().project();
            let buf = Vec::from(&buf[..buf.len()]);
            // Send data through the channel - this should always be ready for unbounded channels
            let Ok(_) = this.write_tx.send(buf) else {
                error!("UnbufferedStream::poll_write failed - channel closed");
                return Poll::Ready(Err(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "Write channel closed",
                )));
            };
            this.pending_write
                .set(Some(Box::pin(tokio::time::sleep(WRITE_DELAY))))
        }
        let Some(sleep) = self.pending_write.as_mut() else {
            panic!("Sleep should have just been set");
        };
        let sleep = sleep.as_mut();
        let Poll::Pending = Future::poll(sleep, cx) else {
            panic!("Sleep always be pending on first poll")
        };
        Poll::Ready(Ok(len))
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn body_test() {
    init_tracing();
    let (server, client) = UnbufferedStream::new_pair();
    let config = fixture::TestConfig::with_timeout(WRITE_DELAY * 2);
    fixture::run(server, client, config).await;
}
