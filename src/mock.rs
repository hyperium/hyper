use std::cmp;
use std::io::{self, Read, Write};

use futures::Async;
use tokio::io::Io;

#[derive(Debug)]
pub struct Buf {
    vec: Vec<u8>,
    pos: usize,
}

impl Buf {
    pub fn new() -> Buf {
        Buf::wrap(vec![])
    }

    pub fn wrap(vec: Vec<u8>) -> Buf {
        Buf {
            vec: vec,
            pos: 0,
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
        (&self.vec[self.pos..]).read(buf).map(|n| {
            self.pos += n;
            n
        })
    }
}

#[derive(Debug)]
pub struct AsyncIo<T> {
    inner: T,
    bytes_until_block: usize,
    error: Option<io::Error>,
}

impl<T> AsyncIo<T> {
    pub fn new(inner: T, bytes: usize) -> AsyncIo<T> {
        AsyncIo {
            inner: inner,
            bytes_until_block: bytes,
            error: None,
        }
    }

    pub fn block_in(&mut self, bytes: usize) {
        self.bytes_until_block = bytes;
    }

    pub fn error(&mut self, err: io::Error) {
        self.error = Some(err);
    }
}

impl AsyncIo<Buf> {
    pub fn new_buf<T: Into<Vec<u8>>>(buf: T, bytes: usize) -> AsyncIo<Buf> {
        AsyncIo::new(Buf::wrap(buf.into()), bytes)
    }
}

impl<T: Read> Read for AsyncIo<T> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if let Some(err) = self.error.take() {
            Err(err)
        } else if self.bytes_until_block == 0 {
            Err(io::Error::new(io::ErrorKind::WouldBlock, "mock block"))
        } else {
            let n = cmp::min(self.bytes_until_block, buf.len());
            let n = try!(self.inner.read(&mut buf[..n]));
            self.bytes_until_block -= n;
            Ok(n)
        }
    }
}

impl<T: Write> Write for AsyncIo<T> {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        if let Some(err) = self.error.take() {
            Err(err)
        } else if self.bytes_until_block == 0 {
            Err(io::Error::new(io::ErrorKind::WouldBlock, "mock block"))
        } else {
            trace!("AsyncIo::write() block_in = {}, data.len() = {}", self.bytes_until_block, data.len());
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

impl<T: Read + Write> Io for AsyncIo<T> {
    fn poll_read(&mut self) -> Async<()> {
        if self.bytes_until_block == 0 {
            Async::NotReady
        } else {
            Async::Ready(())
        }
    }

    fn poll_write(&mut self) -> Async<()> {
        if self.bytes_until_block == 0 {
            Async::NotReady
        } else {
            Async::Ready(())
        }
    }
}

impl ::std::ops::Deref for AsyncIo<Buf> {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        &self.inner
    }
}
