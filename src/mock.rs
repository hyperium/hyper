use std::cmp;
use std::io::{self, Read, Write};

#[derive(Debug)]
pub struct Buf {
    vec: Vec<u8>
}

impl Buf {
    pub fn new() -> Buf {
        Buf {
            vec: vec![]
        }
    }
}

impl ::std::ops::Deref for Buf {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        &self.vec
    }
}

impl<S: AsRef<[u8]>> PartialEq<S> for Buf {
    fn eq(&self, other: &S) -> bool {
        self.vec == other.as_ref()
    }
}

impl Write for Buf {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        self.vec.extend(data);
        Ok(data.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Read for Buf {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        (&*self.vec).read(buf)
    }
}

impl ::vecio::Writev for Buf {
    fn writev(&mut self, bufs: &[&[u8]]) -> io::Result<usize> {
        let cap = bufs.iter().map(|buf| buf.len()).fold(0, |total, next| total + next);
        let mut vec = Vec::with_capacity(cap);
        for &buf in bufs {
            vec.extend(buf);
        }

        self.write(&vec)
    }
}

#[derive(Debug)]
pub struct Async<T> {
    inner: T,
    bytes_until_block: usize,
}

impl<T> Async<T> {
    pub fn new(inner: T, bytes: usize) -> Async<T> {
        Async {
            inner: inner,
            bytes_until_block: bytes
        }
    }

    pub fn block_in(&mut self, bytes: usize) {
        self.bytes_until_block = bytes;
    }
}

impl<T: Read> Read for Async<T> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.bytes_until_block == 0 {
            Err(io::Error::new(io::ErrorKind::WouldBlock, "mock block"))
        } else {
            let n = cmp::min(self.bytes_until_block, buf.len());
            let n = try!(self.inner.read(&mut buf[..n]));
            self.bytes_until_block -= n;
            Ok(n)
        }
    }
}

impl<T: Write> Write for Async<T> {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        if self.bytes_until_block == 0 {
            Err(io::Error::new(io::ErrorKind::WouldBlock, "mock block"))
        } else {
            let n = cmp::min(self.bytes_until_block, data.len());
            let n = try!(self.inner.write(&data[..n]));
            self.bytes_until_block -= n;
            Ok(n)
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

impl<T: Write> ::vecio::Writev for Async<T> {
    fn writev(&mut self, bufs: &[&[u8]]) -> io::Result<usize> {
        let cap = bufs.iter().map(|buf| buf.len()).fold(0, |total, next| total + next);
        let mut vec = Vec::with_capacity(cap);
        for &buf in bufs {
            vec.extend(buf);
        }

        self.write(&vec)
    }
}

impl ::std::ops::Deref for Async<Buf> {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        &self.inner
    }
}
