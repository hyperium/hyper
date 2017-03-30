use std::cmp;
use std::io::{self, Read, BufRead};

pub struct BufReader<R> {
    inner: R,
    buf: Vec<u8>,
    pos: usize,
    cap: usize,
}

const INIT_BUFFER_SIZE: usize = 4096;
const MAX_BUFFER_SIZE: usize = 8192 + 4096 * 100;

impl<R: Read> BufReader<R> {
    #[inline]
    pub fn new(rdr: R) -> BufReader<R> {
        BufReader::with_capacity(rdr, INIT_BUFFER_SIZE)
    }

    #[inline]
    pub fn from_parts(rdr: R, buf: Vec<u8>, pos: usize, cap: usize) -> BufReader<R> {
        BufReader {
            inner: rdr,
            buf: buf,
            pos: pos,
            cap: cap,
        }
    }

    #[inline]
    pub fn with_capacity(rdr: R, cap: usize) -> BufReader<R> {
        BufReader {
            inner: rdr,
            buf: vec![0; cap],
            pos: 0,
            cap: 0,
        }
    }

    #[inline]
    pub fn get_ref(&self) -> &R { &self.inner }

    #[inline]
    pub fn get_mut(&mut self) -> &mut R { &mut self.inner }

    #[inline]
    pub fn get_buf(&self) -> &[u8] {
        if self.pos < self.cap {
            trace!("get_buf [u8; {}][{}..{}]", self.buf.len(), self.pos, self.cap);
            &self.buf[self.pos..self.cap]
        } else {
            trace!("get_buf []");
            &[]
        }
    }

    /// Extracts the buffer from this reader. Return the current cursor position
    /// and the position of the last valid byte.
    ///
    /// This operation does not copy the buffer. Instead, it directly returns
    /// the internal buffer. As a result, this reader will no longer have any
    /// buffered contents and any subsequent read from this reader will not
    /// include the returned buffered contents.
    #[inline]
    pub fn take_buf(&mut self) -> (Vec<u8>, usize, usize) {
        let (pos, cap) = (self.pos, self.cap);
        self.pos = 0;
        self.cap = 0;

        let mut output = vec![0; INIT_BUFFER_SIZE];
        ::std::mem::swap(&mut self.buf, &mut output);
        (output, pos, cap)
    }

    #[inline]
    pub fn into_inner(self) -> R { self.inner }

    #[inline]
    pub fn into_parts(self) -> (R, Vec<u8>, usize, usize) {
        (self.inner, self.buf, self.pos, self.cap)
    }

    #[inline]
    pub fn read_into_buf(&mut self) -> io::Result<usize> {
        self.maybe_reserve();
        let v = &mut self.buf;
        trace!("read_into_buf buf[{}..{}]", self.cap, v.len());
        if self.cap < v.capacity() {
            let nread = try!(self.inner.read(&mut v[self.cap..]));
            self.cap += nread;
            Ok(nread)
        } else {
            trace!("read_into_buf at full capacity");
            Ok(0)
        }
    }

    #[inline]
    fn maybe_reserve(&mut self) {
        let cap = self.buf.capacity();
        if self.cap == cap && cap < MAX_BUFFER_SIZE {
            self.buf.reserve(cmp::min(cap * 4, MAX_BUFFER_SIZE) - cap);
            let new = self.buf.capacity() - self.buf.len();
            trace!("reserved {}", new);
            unsafe { grow_zerofill(&mut self.buf, new) }
        }
    }
}

#[inline]
unsafe fn grow_zerofill(buf: &mut Vec<u8>, additional: usize) {
    use std::ptr;
    let len = buf.len();
    buf.set_len(len + additional);
    ptr::write_bytes(buf.as_mut_ptr().offset(len as isize), 0, additional);
}

impl<R: Read> Read for BufReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.cap == self.pos && buf.len() >= self.buf.len() {
            return self.inner.read(buf);
        }
        let nread = {
           let mut rem = try!(self.fill_buf());
           try!(rem.read(buf))
        };
        self.consume(nread);
        Ok(nread)
    }
}

impl<R: Read> BufRead for BufReader<R> {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        if self.pos == self.cap {
            self.cap = try!(self.inner.read(&mut self.buf));
            self.pos = 0;
        }
        Ok(&self.buf[self.pos..self.cap])
    }

    #[inline]
    fn consume(&mut self, amt: usize) {
        self.pos = cmp::min(self.pos + amt, self.cap);
        if self.pos == self.cap {
            self.pos = 0;
            self.cap = 0;
        }
    }
}

#[cfg(test)]
mod tests {

    use std::io::{self, Read, BufRead};
    use super::BufReader;

    struct SlowRead(u8);

    impl Read for SlowRead {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            let state = self.0;
            self.0 += 1;
            (&match state % 3 {
                0 => b"foo",
                1 => b"bar",
                _ => b"baz",
            }[..]).read(buf)
        }
    }

    #[test]
    fn test_consume_and_get_buf() {
        let mut rdr = BufReader::new(SlowRead(0));
        rdr.read_into_buf().unwrap();
        rdr.consume(1);
        assert_eq!(rdr.get_buf(), b"oo");
        rdr.read_into_buf().unwrap();
        rdr.read_into_buf().unwrap();
        assert_eq!(rdr.get_buf(), b"oobarbaz");
        rdr.consume(5);
        assert_eq!(rdr.get_buf(), b"baz");
        rdr.consume(3);
        assert_eq!(rdr.get_buf(), b"");
        assert_eq!(rdr.pos, 0);
        assert_eq!(rdr.cap, 0);
    }

    #[test]
    fn test_resize() {
        let raw = b"hello world";
        let mut rdr = BufReader::with_capacity(&raw[..], 5);
        rdr.read_into_buf().unwrap();
        assert_eq!(rdr.get_buf(), b"hello");
        rdr.read_into_buf().unwrap();
        assert_eq!(rdr.get_buf(), b"hello world");
    }
}
