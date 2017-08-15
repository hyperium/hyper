use std::cmp;
use std::fmt;
use std::io::{self, Write};
use std::ptr;

use futures::{Async, Poll};
use tokio_io::{AsyncRead, AsyncWrite};

use http::{Http1Transaction, MessageHead};
use bytes::{BytesMut, Bytes};

const INIT_BUFFER_SIZE: usize = 8192;
pub const MAX_BUFFER_SIZE: usize = 8192 + 4096 * 100;

pub struct Buffered<T> {
    io: T,
    read_blocked: bool,
    read_buf: BytesMut,
    write_buf: WriteBuf,
}

impl<T> fmt::Debug for Buffered<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Buffered")
            .field("read_buf", &self.read_buf)
            .field("write_buf", &self.write_buf)
            .finish()
    }
}

impl<T: AsyncRead + AsyncWrite> Buffered<T> {
    pub fn new(io: T) -> Buffered<T> {
        Buffered {
            io: io,
            read_buf: BytesMut::with_capacity(0),
            write_buf: WriteBuf::new(),
            read_blocked: false,
        }
    }

    pub fn read_buf(&self) -> &[u8] {
        self.read_buf.as_ref()
    }

    pub fn write_buf_mut(&mut self) -> &mut Vec<u8> {
        self.write_buf.maybe_reset();
        self.write_buf.maybe_reserve(0);
        &mut self.write_buf.0.bytes
    }

    pub fn consume_leading_lines(&mut self) {
        if !self.read_buf.is_empty() {
            let mut i = 0;
            while i < self.read_buf.len() {
                match self.read_buf[i] {
                    b'\r' | b'\n' => i += 1,
                    _ => break,
                }
            }
            self.read_buf.split_to(i);
        }
    }

    pub fn parse<S: Http1Transaction>(&mut self) -> Poll<MessageHead<S::Incoming>, ::Error> {
        loop {
            match try!(S::parse(&mut self.read_buf)) {
                Some(head) => {
                    //trace!("parsed {} bytes out of {}", len, self.read_buf.len());
                    return Ok(Async::Ready(head.0))
                },
                None => {
                    if self.read_buf.capacity() >= MAX_BUFFER_SIZE {
                        debug!("MAX_BUFFER_SIZE reached, closing");
                        return Err(::Error::TooLarge);
                    }
                },
            }
            match try_ready!(self.read_from_io()) {
                0 => {
                    trace!("parse eof");
                    //TODO: With Rust 1.14, this can be Error::from(ErrorKind)
                    return Err(io::Error::new(io::ErrorKind::UnexpectedEof, ParseEof).into());
                }
                _ => {},
            }
        }
    }

    fn read_from_io(&mut self) -> Poll<usize, io::Error> {
        use bytes::BufMut;
        // TODO: Investigate if we still need these unsafe blocks
        if self.read_buf.remaining_mut() < INIT_BUFFER_SIZE {
            self.read_buf.reserve(INIT_BUFFER_SIZE);
            unsafe { // Zero out unused memory
                let buf = self.read_buf.bytes_mut();
                let len = buf.len();
                ptr::write_bytes(buf.as_mut_ptr(), 0, len);
            }
        }
        self.read_blocked = false;
        unsafe { // Can we use AsyncRead::read_buf instead?
            let n = match self.io.read(self.read_buf.bytes_mut()) {
                Ok(n) => n,
                Err(e) => {
                    if e.kind() == io::ErrorKind::WouldBlock {
                        // TODO: Push this out, ideally, into http::Conn.
                        self.read_blocked = true;
                        return Ok(Async::NotReady);
                    }
                    return Err(e)
                }
            };
            self.read_buf.advance_mut(n);
            Ok(Async::Ready(n))
        }
    }

    pub fn buffer<B: AsRef<[u8]>>(&mut self, buf: B) -> usize {
        self.write_buf.buffer(buf.as_ref())
    }

    pub fn io_mut(&mut self) -> &mut T {
        &mut self.io
    }

    pub fn is_read_blocked(&self) -> bool {
        self.read_blocked
    }
}

impl<T: Write> Write for Buffered<T> {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        let n = self.write_buf.buffer(data);
        if n == 0 {
            Err(io::Error::from(io::ErrorKind::WouldBlock))
        } else {
            Ok(n)
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.write_buf.remaining() == 0 {
            self.io.flush()
        } else {
            loop {
                let n = try!(self.write_buf.write_into(&mut self.io));
                trace!("flushed {} bytes", n);
                if self.write_buf.remaining() == 0 {
                    break;
                }
            }
            self.io.flush()
        }
    }
}

pub trait MemRead {
    fn read_mem(&mut self, len: usize) -> Poll<Bytes, io::Error>;
}

impl<T: AsyncRead + AsyncWrite> MemRead for Buffered<T> {
    fn read_mem(&mut self, len: usize) -> Poll<Bytes, io::Error> {
        trace!("Buffered.read_mem read_buf={}, wanted={}", self.read_buf.len(), len);
        if !self.read_buf.is_empty() {
            let n = ::std::cmp::min(len, self.read_buf.len());
            trace!("Buffered.read_mem read_buf is not empty, slicing {}", n);
            Ok(Async::Ready(self.read_buf.split_to(n).freeze()))
        } else {
            let n = try_ready!(self.read_from_io());
            Ok(Async::Ready(self.read_buf.split_to(::std::cmp::min(len, n)).freeze()))
        }
    }
}

#[derive(Clone)]
pub struct Cursor<T> {
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

    pub fn has_started(&self) -> bool {
        self.pos != 0
    }

    pub fn is_written(&self) -> bool {
        trace!("Cursor::is_written pos = {}, len = {}", self.pos, self.bytes.as_ref().len());
        self.pos >= self.bytes.as_ref().len()
    }

    pub fn write_to<W: Write>(&mut self, dst: &mut W) -> io::Result<usize> {
        if self.remaining() == 0 {
            Ok(0)
        } else {
            dst.write(&self.bytes.as_ref()[self.pos..]).map(|n| {
                self.pos += n;
                n
            })
        }
    }

    fn remaining(&self) -> usize {
        self.bytes.as_ref().len() - self.pos
    }

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
        f.debug_struct("Cursor")
            .field("pos", &self.pos)
            .field("len", &self.bytes.as_ref().len())
            .finish()
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
        if bufs.len() == 1 {
            self.write(bufs[0])
        } else {
            let vec = bufs.concat();
            self.write(&vec)
        }
    }
}
//}

// an internal buffer to collect writes before flushes
#[derive(Debug)]
struct WriteBuf(Cursor<Vec<u8>>);

impl WriteBuf {
    fn new() -> WriteBuf {
        WriteBuf(Cursor::new(Vec::new()))
    }

    fn write_into<W: Write>(&mut self, w: &mut W) -> io::Result<usize> {
        self.0.write_to(w)
    }

    fn buffer(&mut self, data: &[u8]) -> usize {
        trace!("WriteBuf::buffer() len = {:?}", data.len());
        self.maybe_reset();
        self.maybe_reserve(data.len());
        let vec = &mut self.0.bytes;
        let len = cmp::min(vec.capacity() - vec.len(), data.len());
        assert!(vec.capacity() - vec.len() >= len);
        unsafe {
            // in rust 1.9, we could use slice::copy_from_slice
            ptr::copy(
                data.as_ptr(),
                vec.as_mut_ptr().offset(vec.len() as isize),
                len
            );
            let new_len = vec.len() + len;
            vec.set_len(new_len);
        }
        len
    }

    fn remaining(&self) -> usize {
        self.0.remaining()
    }

    #[inline]
    fn maybe_reserve(&mut self, needed: usize) {
        let vec = &mut self.0.bytes;
        let cap = vec.capacity();
        if cap == 0 {
            let init = cmp::min(MAX_BUFFER_SIZE, cmp::max(INIT_BUFFER_SIZE, needed));
            trace!("WriteBuf reserving initial {}", init);
            vec.reserve(init);
        } else if cap < MAX_BUFFER_SIZE {
            vec.reserve(cmp::min(needed, MAX_BUFFER_SIZE - cap));
            trace!("WriteBuf reserved {}", vec.capacity() - cap);
        }
    }

    fn maybe_reset(&mut self) {
        if self.0.pos != 0 && self.0.remaining() == 0 {
            self.0.pos = 0;
            unsafe {
                self.0.bytes.set_len(0);
            }
        }
    }
}

#[derive(Debug)]
struct ParseEof;

impl fmt::Display for ParseEof {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("parse eof")
    }
}

impl ::std::error::Error for ParseEof {
    fn description(&self) -> &str {
        "parse eof"
    }
}

// TODO: Move tests to their own mod
#[cfg(test)]
use std::io::Read;

#[cfg(test)]
impl<T: Read> MemRead for ::mock::AsyncIo<T> {
    fn read_mem(&mut self, len: usize) -> Poll<Bytes, io::Error> {
        let mut v = vec![0; len];
        let n = try_nb!(self.read(v.as_mut_slice()));
        Ok(Async::Ready(BytesMut::from(&v[..n]).freeze()))
    }
}

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

#[test]
fn test_parse_reads_until_blocked() {
    use mock::{AsyncIo, Buf as MockBuf};
    // missing last line ending
    let raw = "HTTP/1.1 200 OK\r\n";

    let mock = AsyncIo::new(MockBuf::wrap(raw.into()), raw.len());
    let mut buffered = Buffered::new(mock);
    assert_eq!(buffered.parse::<super::ClientTransaction>().unwrap(), Async::NotReady);
    assert!(buffered.io.blocked());
}
