use std::cmp;
use std::io::{self, Read, Write};

use futures::Poll;
use tokio_io::{AsyncRead, AsyncWrite};

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

impl AsRef<[u8]> for Buf {
    fn as_ref(&self) -> &[u8] {
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
    blocked: bool,
    flushed: bool,
}

impl<T> AsyncIo<T> {
    pub fn new(inner: T, bytes: usize) -> AsyncIo<T> {
        AsyncIo {
            inner: inner,
            bytes_until_block: bytes,
            error: None,
            flushed: false,
            blocked: false,
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

    #[cfg(feature = "tokio-proto")]
    //TODO: fix proto::conn::tests to not use tokio-proto API,
    //and then this cfg flag go away
    pub fn new_eof() -> AsyncIo<Buf> {
        AsyncIo::new(Buf::wrap(Vec::new().into()), 1)
    }

    #[cfg(feature = "tokio-proto")]
    //TODO: fix proto::conn::tests to not use tokio-proto API,
    //and then this cfg flag go away
    pub fn flushed(&self) -> bool {
        self.flushed
    }

    pub fn blocked(&self) -> bool {
        self.blocked
    }
}

impl<S: AsRef<[u8]>, T: AsRef<[u8]>> PartialEq<S> for AsyncIo<T> {
    fn eq(&self, other: &S) -> bool {
        self.inner.as_ref() == other.as_ref()
    }
}


impl<T: Read> Read for AsyncIo<T> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.blocked = false;
        if let Some(err) = self.error.take() {
            Err(err)
        } else if self.bytes_until_block == 0 {
            self.blocked = true;
            Err(io::ErrorKind::WouldBlock.into())
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
            Err(io::ErrorKind::WouldBlock.into())
        } else {
            trace!("AsyncIo::write() block_in = {}, data.len() = {}", self.bytes_until_block, data.len());
            self.flushed = false;
            let n = cmp::min(self.bytes_until_block, data.len());
            let n = try!(self.inner.write(&data[..n]));
            self.bytes_until_block -= n;
            Ok(n)
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        self.flushed = true;
        self.inner.flush()
    }
}

impl<T: Read + Write> AsyncRead for AsyncIo<T> {
}

impl<T: Read + Write> AsyncWrite for AsyncIo<T> {
    fn shutdown(&mut self) -> Poll<(), io::Error> {
        Ok(().into())
    }

    fn write_buf<B: ::bytes::Buf>(&mut self, buf: &mut B) -> Poll<usize, io::Error> {
        use futures::Async;
        let r = {
            static DUMMY: &[u8] = &[0];
            let mut bufs = [From::from(DUMMY); 64];
            let i = ::bytes::Buf::bytes_vec(&buf, &mut bufs);
            let mut n = 0;
            let mut ret = Ok(0);
            for iovec in &bufs[..i] {
                match self.write(iovec) {
                    Ok(num) => {
                        n += num;
                        ret = Ok(n);
                    },
                    Err(e) => {
                        if e.kind() == io::ErrorKind::WouldBlock {
                            if let Ok(0) = ret {
                                ret = Err(e);
                            }
                        } else {
                            ret = Err(e);
                        }
                        break;
                    }
                }
            }
            ret
        };
        match r {
            Ok(n) => {
                ::bytes::Buf::advance(buf, n);
                Ok(Async::Ready(n))
            }
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                Ok(Async::NotReady)
            }
            Err(e) => Err(e),
        }
    }
}

impl ::std::ops::Deref for AsyncIo<Buf> {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        &self.inner
    }
}
