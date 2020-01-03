use std::marker::Unpin;
use std::{cmp, io};

use bytes::{Buf, Bytes};
use tokio::io::{AsyncRead, AsyncWrite};

use crate::common::{task, Pin, Poll};

/// Combine a buffer with an IO, rewinding reads to use the buffer.
#[derive(Debug)]
pub(crate) struct Rewind<T> {
    pre: Option<Bytes>,
    inner: T,
}

impl<T> Rewind<T> {
    pub(crate) fn new(io: T) -> Self {
        Rewind {
            pre: None,
            inner: io,
        }
    }

    pub(crate) fn new_buffered(io: T, buf: Bytes) -> Self {
        Rewind {
            pre: Some(buf),
            inner: io,
        }
    }

    pub(crate) fn rewind(&mut self, bs: Bytes) {
        debug_assert!(self.pre.is_none());
        self.pre = Some(bs);
    }

    pub(crate) fn into_inner(self) -> (T, Bytes) {
        (self.inner, self.pre.unwrap_or_else(Bytes::new))
    }

    pub(crate) fn get_mut(&mut self) -> &mut T {
        &mut self.inner
    }
}

impl<T> AsyncRead for Rewind<T>
where
    T: AsyncRead + Unpin,
{
    #[inline]
    unsafe fn prepare_uninitialized_buffer(&self, buf: &mut [std::mem::MaybeUninit<u8>]) -> bool {
        self.inner.prepare_uninitialized_buffer(buf)
    }

    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        if let Some(mut prefix) = self.pre.take() {
            // If there are no remaining bytes, let the bytes get dropped.
            if !prefix.is_empty() {
                let copy_len = cmp::min(prefix.len(), buf.len());
                prefix.copy_to_slice(&mut buf[..copy_len]);
                // Put back whats left
                if !prefix.is_empty() {
                    self.pre = Some(prefix);
                }

                return Poll::Ready(Ok(copy_len));
            }
        }
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

impl<T> AsyncWrite for Rewind<T>
where
    T: AsyncWrite + Unpin,
{
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.inner).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }

    #[inline]
    fn poll_write_buf<B: Buf>(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
        buf: &mut B,
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.inner).poll_write_buf(cx, buf)
    }
}

#[cfg(test)]
mod tests {
    // FIXME: re-implement tests with `async/await`, this import should
    // trigger a warning to remind us
    use super::Rewind;
    use bytes::Bytes;
    use tokio::io::AsyncReadExt;

    #[tokio::test]
    async fn partial_rewind() {
        let underlying = [104, 101, 108, 108, 111];

        let mock = tokio_test::io::Builder::new().read(&underlying).build();

        let mut stream = Rewind::new(mock);

        // Read off some bytes, ensure we filled o1
        let mut buf = [0; 2];
        stream.read_exact(&mut buf).await.expect("read1");

        // Rewind the stream so that it is as if we never read in the first place.
        stream.rewind(Bytes::copy_from_slice(&buf[..]));

        let mut buf = [0; 5];
        stream.read_exact(&mut buf).await.expect("read1");

        // At this point we should have read everything that was in the MockStream
        assert_eq!(&buf, &underlying);
    }

    #[tokio::test]
    async fn full_rewind() {
        let underlying = [104, 101, 108, 108, 111];

        let mock = tokio_test::io::Builder::new().read(&underlying).build();

        let mut stream = Rewind::new(mock);

        let mut buf = [0; 5];
        stream.read_exact(&mut buf).await.expect("read1");

        // Rewind the stream so that it is as if we never read in the first place.
        stream.rewind(Bytes::copy_from_slice(&buf[..]));

        let mut buf = [0; 5];
        stream.read_exact(&mut buf).await.expect("read1");
    }
}
