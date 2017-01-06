use std::fmt;
use std::io::{self, Read, Write};

use futures::Async;
use tokio::io::Io;

use http::{Http1Transaction, h1, MessageHead, ParseResult};
use http::buffer::Buffer;

pub struct Buffered<T> {
    io: T,
    read_buf: Buffer,
    write_buf: Buffer,
}

impl<T> fmt::Debug for Buffered<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Buffered")
            .field("read_buf", &self.read_buf)
            .field("write_buf", &self.write_buf)
            .finish()
    }
}

impl<T: Io> Buffered<T> {
    pub fn new(io: T) -> Buffered<T> {
        Buffered {
            io: io,
            read_buf: Buffer::new(),
            write_buf: Buffer::new(),
        }
    }

    pub fn read_buf(&self) -> &[u8] {
        self.read_buf.bytes()
    }

    pub fn consume_leading_lines(&mut self) {
        self.read_buf.consume_leading_lines();
    }

    pub fn poll_read(&mut self) -> Async<()> {
        self.io.poll_read()
    }

    pub fn parse<S: Http1Transaction>(&mut self) -> ::Result<Option<MessageHead<S::Incoming>>> {
        match self.read_buf.read_from(&mut self.io) {
            Ok(0) => {
                trace!("parse eof");
                return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "parse eof").into());
            }
            Ok(_) => {},
            Err(e) => match e.kind() {
                io::ErrorKind::WouldBlock => {},
                _ => return Err(e.into())
            }
        }
        match try!(parse::<S, _>(self.read_buf.bytes())) {
            Some((head, len)) => {
                trace!("parsed {} bytes out of {}", len, self.read_buf.len());
                self.read_buf.consume(len);
                Ok(Some(head))
            },
            None => {
                if self.read_buf.is_max_size() {
                    debug!("MAX_BUFFER_SIZE reached, closing");
                    Err(::Error::TooLarge)
                } else {
                    Ok(None)
                }
            },
        }
    }

    pub fn buffer<B: AsRef<[u8]>>(&mut self, buf: B) {
        self.write_buf.write(buf.as_ref());
    }

    #[cfg(test)]
    pub fn io_mut(&mut self) -> &mut T {
        &mut self.io
    }
}

impl<T: Read> Read for Buffered<T> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        trace!("Buffered.read self={}, buf={}", self.read_buf.len(), buf.len());
        let n = try!(self.read_buf.bytes().read(buf));
        self.read_buf.consume(n);
        if n == 0 {
            self.read_buf.reset();
            self.io.read(&mut buf[n..])
        } else {
            Ok(n)
        }
    }
}

impl<T: Write> Write for Buffered<T> {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        Ok(self.write_buf.write(data))
    }

    fn flush(&mut self) -> io::Result<()> {
        self.write_buf.write_into(&mut self.io).and_then(|_n| {
            if self.write_buf.is_empty() {
                Ok(())
            } else {
                Err(io::Error::new(io::ErrorKind::WouldBlock, "wouldblock"))
            }
        })
    }
}
fn parse<T: Http1Transaction<Incoming=I>, I>(rdr: &[u8]) -> ParseResult<I> {
    h1::parse::<T, I>(rdr)
}

#[derive(Clone)]
pub struct Cursor<T: AsRef<[u8]>> {
    bytes: T,
    pos: usize,
}

impl<T: AsRef<[u8]>> Cursor<T> {
    pub fn new(bytes: T) -> Cursor<T> {
        Cursor {
            bytes: bytes,
            pos: 0,
        }
    }

    pub fn is_written(&self) -> bool {
        trace!("Cursor::is_written pos = {}, len = {}", self.pos, self.bytes.as_ref().len());
        self.pos >= self.bytes.as_ref().len()
    }

    /*
    pub fn write_to<W: Write>(&mut self, dst: &mut W) -> io::Result<usize> {
        dst.write(&self.bytes.as_ref()[self.pos..]).map(|n| {
            self.pos += n;
            n
        })
    }
    */

    #[inline]
    pub fn buf(&self) -> &[u8] {
        &self.bytes.as_ref()[self.pos..]
    }

    #[inline]
    pub fn consume(&mut self, num: usize) {
        trace!("Cursor::consume({})", num);
        self.pos = ::std::cmp::min(self.bytes.as_ref().len(), self.pos + num);
    }
}

impl<T: AsRef<[u8]>> fmt::Debug for Cursor<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let bytes = self.buf();
        let reasonable_max = ::std::cmp::min(bytes.len(), 32);
        write!(f, "Cursor({:?})", &bytes[..reasonable_max])
    }
}

pub trait AtomicWrite {
    fn write_atomic(&mut self, data: &[&[u8]]) -> io::Result<usize>;
}

/*
#[cfg(not(windows))]
impl<T: Write + ::vecio::Writev> AtomicWrite for T {

    fn write_atomic(&mut self, bufs: &[&[u8]]) -> io::Result<usize> {
        self.writev(bufs)
    }

}

#[cfg(windows)]
*/
impl<T: Write> AtomicWrite for T {
    fn write_atomic(&mut self, bufs: &[&[u8]]) -> io::Result<usize> {
        if cfg!(not(windows)) {
            warn!("write_atomic not using writev");
        }
        let vec = bufs.concat();
        self.write(&vec)
    }
}
//}


#[test]
fn test_iobuf_write_empty_slice() {
    use mock::{AsyncIo, Buf as MockBuf};

    let mut mock = AsyncIo::new(MockBuf::new(), 256);
    mock.error(io::Error::new(io::ErrorKind::Other, "logic error"));

    let mut io_buf = Buffered::new(mock);

    // underlying io will return the logic error upon write,
    // so we are testing that the io_buf does not trigger a write
    // when there is nothing to flush
    io_buf.flush().expect("should short-circuit flush");
}
