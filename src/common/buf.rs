use std::collections::VecDeque;
use std::io::IoSlice;

use bytes::{Buf, BufMut, Bytes, BytesMut};

pub(crate) struct BufList<T> {
    bufs: VecDeque<T>,
}

/// A `Buf` that can expose a contiguous prefix as vectored slices.
///
/// Unlike `Buf::chunks_vectored`, this method guarantees that every returned
/// slice follows the previous one in the buffer's logical byte stream. The
/// boolean indicates whether the slices cover all remaining bytes.
pub(crate) trait VectoredBuf: Buf {
    #[inline]
    fn chunks_vectored_prefix<'t>(&'t self, dst: &mut [IoSlice<'t>]) -> (usize, bool) {
        chunks_vectored_from_chunk(self, dst)
    }
}

/// Fill one vectored slice from the only prefix primitive guaranteed by `Buf`.
#[inline]
pub(crate) fn chunks_vectored_from_chunk<'t, B>(
    buf: &'t B,
    dst: &mut [IoSlice<'t>],
) -> (usize, bool)
where
    B: Buf + ?Sized,
{
    let remaining = buf.remaining();
    if remaining == 0 {
        return (0, true);
    }
    if dst.is_empty() {
        return (0, false);
    }

    let chunk = buf.chunk();
    if chunk.is_empty() {
        return (0, false);
    }
    dst[0] = IoSlice::new(chunk);
    (1, chunk.len() == remaining)
}

impl<T: Buf> BufList<T> {
    pub(crate) fn new() -> BufList<T> {
        BufList {
            bufs: VecDeque::new(),
        }
    }

    #[inline]
    pub(crate) fn push(&mut self, buf: T) {
        debug_assert!(buf.has_remaining());
        self.bufs.push_back(buf);
    }

    #[inline]
    pub(crate) fn bufs_cnt(&self) -> usize {
        self.bufs.len()
    }
}

impl<T: Buf> Buf for BufList<T> {
    #[inline]
    fn remaining(&self) -> usize {
        self.bufs.iter().map(|buf| buf.remaining()).sum()
    }

    #[inline]
    fn chunk(&self) -> &[u8] {
        self.bufs.front().map(Buf::chunk).unwrap_or_default()
    }

    #[inline]
    fn advance(&mut self, mut cnt: usize) {
        while cnt > 0 {
            {
                let front = &mut self.bufs[0];
                let rem = front.remaining();
                if rem > cnt {
                    front.advance(cnt);
                    return;
                } else {
                    front.advance(rem);
                    cnt -= rem;
                }
            }
            self.bufs.pop_front();
        }
    }

    #[inline]
    fn copy_to_bytes(&mut self, len: usize) -> Bytes {
        // Our inner buffer may have an optimized version of copy_to_bytes, and if the whole
        // request can be fulfilled by the front buffer, we can take advantage.
        match self.bufs.front_mut() {
            Some(front) if front.remaining() == len => {
                let b = front.copy_to_bytes(len);
                self.bufs.pop_front();
                b
            }
            Some(front) if front.remaining() > len => front.copy_to_bytes(len),
            _ => {
                assert!(len <= self.remaining(), "`len` greater than remaining");
                let mut bm = BytesMut::with_capacity(len);
                bm.put(self.take(len));
                bm.freeze()
            }
        }
    }
}

impl<T: VectoredBuf> VectoredBuf for BufList<T> {
    #[inline]
    fn chunks_vectored_prefix<'t>(&'t self, dst: &mut [IoSlice<'t>]) -> (usize, bool) {
        let mut vecs = 0;
        for buf in &self.bufs {
            if vecs == dst.len() {
                return (vecs, false);
            }
            let (n, complete) = buf.chunks_vectored_prefix(&mut dst[vecs..]);
            vecs += n;
            if !complete {
                return (vecs, false);
            }
        }
        (vecs, true)
    }
}

#[cfg(test)]
mod tests {
    use std::ptr;

    use super::*;

    fn hello_world_buf() -> BufList<Bytes> {
        BufList {
            bufs: vec![Bytes::from("Hello"), Bytes::from(" "), Bytes::from("World")].into(),
        }
    }

    #[test]
    fn to_bytes_shorter() {
        let mut bufs = hello_world_buf();
        let old_ptr = bufs.chunk().as_ptr();
        let start = bufs.copy_to_bytes(4);
        assert_eq!(start, "Hell");
        assert!(ptr::eq(old_ptr, start.as_ptr()));
        assert_eq!(bufs.chunk(), b"o");
        assert!(ptr::eq(old_ptr.wrapping_add(4), bufs.chunk().as_ptr()));
        assert_eq!(bufs.remaining(), 7);
    }

    #[test]
    fn to_bytes_eq() {
        let mut bufs = hello_world_buf();
        let old_ptr = bufs.chunk().as_ptr();
        let start = bufs.copy_to_bytes(5);
        assert_eq!(start, "Hello");
        assert!(ptr::eq(old_ptr, start.as_ptr()));
        assert_eq!(bufs.chunk(), b" ");
        assert_eq!(bufs.remaining(), 6);
    }

    #[test]
    fn to_bytes_longer() {
        let mut bufs = hello_world_buf();
        let start = bufs.copy_to_bytes(7);
        assert_eq!(start, "Hello W");
        assert_eq!(bufs.remaining(), 4);
    }

    #[test]
    fn one_long_buf_to_bytes() {
        let mut buf = BufList::new();
        buf.push(b"Hello World" as &[_]);
        assert_eq!(buf.copy_to_bytes(5), "Hello");
        assert_eq!(buf.chunk(), b" World");
    }

    #[test]
    #[should_panic(expected = "`len` greater than remaining")]
    fn buf_to_bytes_too_many() {
        hello_world_buf().copy_to_bytes(42);
    }
}
