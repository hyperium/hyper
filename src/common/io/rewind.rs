use std::io::{self, Read};
use std::marker::Unpin;

use bytes::{Buf, Bytes, IntoBuf};
use tokio_io::{AsyncRead, AsyncWrite};

use crate::common::{Pin, Poll, task};

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
}

impl<T> AsyncRead for Rewind<T>
where
    T: AsyncRead + Unpin,
{
    #[inline]
    unsafe fn prepare_uninitialized_buffer(&self, buf: &mut [u8]) -> bool {
        self.inner.prepare_uninitialized_buffer(buf)
    }

    fn poll_read(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>, buf: &mut [u8]) -> Poll<io::Result<usize>> {
        if let Some(pre_bs) = self.pre.take() {
            // If there are no remaining bytes, let the bytes get dropped.
            if pre_bs.len() > 0 {
                let mut pre_reader = pre_bs.into_buf().reader();
                let read_cnt = pre_reader.read(buf)?;

                let mut new_pre = pre_reader.into_inner().into_inner();
                new_pre.advance(read_cnt);

                // Put back whats left
                if new_pre.len() > 0 {
                    self.pre = Some(new_pre);
                }

                return Poll::Ready(Ok(read_cnt));
            }
        }
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

impl<T> AsyncWrite for Rewind<T>
where
    T: AsyncWrite + Unpin,
{
    fn poll_write(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.inner).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }

    #[inline]
    fn poll_write_buf<B: Buf>(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>, buf: &mut B) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.inner).poll_write_buf(cx, buf)
    }
}

#[cfg(test)]
mod tests {
    // FIXME: re-implement tests with `async/await`, this import should
    // trigger a warning to remind us
    use super::Rewind;
    /*
    use super::*;
    use tokio_mockstream::MockStream;
    use std::io::Cursor;

    // Test a partial rewind
    #[test]
    fn async_partial_rewind() {
        let bs = &mut [104, 101, 108, 108, 111];
        let o1 = &mut [0, 0];
        let o2 = &mut [0, 0, 0, 0, 0];

        let mut stream = Rewind::new(MockStream::new(bs));
        let mut o1_cursor = Cursor::new(o1);
        // Read off some bytes, ensure we filled o1
        match stream.read_buf(&mut o1_cursor).unwrap() {
            Async::NotReady => panic!("should be ready"),
            Async::Ready(cnt) => assert_eq!(2, cnt),
        }

        // Rewind the stream so that it is as if we never read in the first place.
        let read_buf = Bytes::from(&o1_cursor.into_inner()[..]);
        stream.rewind(read_buf);

        // We poll 2x here since the first time we'll only get what is in the
        // prefix (the rewinded part) of the Rewind.\
        let mut o2_cursor = Cursor::new(o2);
        stream.read_buf(&mut o2_cursor).unwrap();
        stream.read_buf(&mut o2_cursor).unwrap();
        let o2_final = o2_cursor.into_inner();

        // At this point we should have read everything that was in the MockStream
        assert_eq!(&o2_final, &bs);
    }
    // Test a full rewind
    #[test]
    fn async_full_rewind() {
        let bs = &mut [104, 101, 108, 108, 111];
        let o1 = &mut [0, 0, 0, 0, 0];
        let o2 = &mut [0, 0, 0, 0, 0];

        let mut stream = Rewind::new(MockStream::new(bs));
        let mut o1_cursor = Cursor::new(o1);
        match stream.read_buf(&mut o1_cursor).unwrap() {
            Async::NotReady => panic!("should be ready"),
            Async::Ready(cnt) => assert_eq!(5, cnt),
        }

        let read_buf = Bytes::from(&o1_cursor.into_inner()[..]);
        stream.rewind(read_buf);

        let mut o2_cursor = Cursor::new(o2);
        stream.read_buf(&mut o2_cursor).unwrap();
        stream.read_buf(&mut o2_cursor).unwrap();
        let o2_final = o2_cursor.into_inner();

        assert_eq!(&o2_final, &bs);
    }
    #[test]
    fn partial_rewind() {
        let bs = &mut [104, 101, 108, 108, 111];
        let o1 = &mut [0, 0];
        let o2 = &mut [0, 0, 0, 0, 0];

        let mut stream = Rewind::new(MockStream::new(bs));
        stream.read(o1).unwrap();

        let read_buf = Bytes::from(&o1[..]);
        stream.rewind(read_buf);
        let cnt = stream.read(o2).unwrap();
        stream.read(&mut o2[cnt..]).unwrap();
        assert_eq!(&o2, &bs);
    }
    #[test]
    fn full_rewind() {
        let bs = &mut [104, 101, 108, 108, 111];
        let o1 = &mut [0, 0, 0, 0, 0];
        let o2 = &mut [0, 0, 0, 0, 0];

        let mut stream = Rewind::new(MockStream::new(bs));
        stream.read(o1).unwrap();

        let read_buf = Bytes::from(&o1[..]);
        stream.rewind(read_buf);
        let cnt = stream.read(o2).unwrap();
        stream.read(&mut o2[cnt..]).unwrap();
        assert_eq!(&o2, &bs);
    }
    */
}
