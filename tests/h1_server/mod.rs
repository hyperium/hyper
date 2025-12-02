pub mod fixture;

use hyper::rt::{Read, ReadBufCursor};
use pin_project_lite::pin_project;
use std::io;
use std::pin::Pin;
use std::task::{ready, Context, Poll};
use tokio::sync::mpsc;

// Common read half shared by both stream types
pin_project! {
    #[derive(Debug)]
    pub struct StreamReadHalf {
        #[pin]
        read_rx: mpsc::UnboundedReceiver<Vec<u8>>,
        read_buffer: Vec<u8>,
    }
}

impl StreamReadHalf {
    pub fn new(read_rx: mpsc::UnboundedReceiver<Vec<u8>>) -> Self {
        Self {
            read_rx,
            read_buffer: Vec::new(),
        }
    }
}

impl Read for StreamReadHalf {
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
        match this.read_rx.as_mut().get_mut().try_recv() {
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

pub fn init_tracing() {
    use std::sync::Once;
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::INFO)
            .with_target(true)
            .with_thread_ids(true)
            .with_thread_names(true)
            .init();
    });
}
