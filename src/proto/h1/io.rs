use std::collections::VecDeque;
use std::fmt;
use std::io;

use bytes::{Buf, BufMut, Bytes, BytesMut};
use futures::{Async, Poll};
use iovec::IoVec;
use tokio_io::{AsyncRead, AsyncWrite};

use proto::{Http1Transaction, MessageHead};

const INIT_BUFFER_SIZE: usize = 8192;
pub const DEFAULT_MAX_BUFFER_SIZE: usize = 8192 + 4096 * 100;
const MAX_BUF_LIST_BUFFERS: usize = 16;

pub struct Buffered<T, B> {
    flush_pipeline: bool,
    io: T,
    max_buf_size: usize,
    read_blocked: bool,
    read_buf: BytesMut,
    write_buf: WriteBuf<B>,
}

impl<T, B> fmt::Debug for Buffered<T, B>
where
    B: Buf,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Buffered")
            .field("read_buf", &self.read_buf)
            .field("write_buf", &self.write_buf)
            .finish()
    }
}

impl<T, B> Buffered<T, B>
where
    T: AsyncRead + AsyncWrite,
    B: Buf,
{
    pub fn new(io: T) -> Buffered<T, B> {
        Buffered {
            flush_pipeline: false,
            io: io,
            max_buf_size: DEFAULT_MAX_BUFFER_SIZE,
            read_buf: BytesMut::with_capacity(0),
            write_buf: WriteBuf::new(),
            read_blocked: false,
        }
    }

    pub fn set_flush_pipeline(&mut self, enabled: bool) {
        self.flush_pipeline = enabled;
        self.write_buf.set_strategy(if enabled {
            Strategy::Flatten
        } else {
            Strategy::Queue
        });
    }

    pub fn set_max_buf_size(&mut self, max: usize) {
        self.max_buf_size = max;
        self.write_buf.max_buf_size = max;
    }

    pub fn read_buf(&self) -> &[u8] {
        self.read_buf.as_ref()
    }

    pub fn write_buf_mut(&mut self) -> &mut Vec<u8> {
        let buf = self.write_buf.head_mut();
        buf.maybe_reset();
        &mut buf.bytes
    }

    pub fn buffer(&mut self, buf: B) {
        self.write_buf.buffer(buf)
    }

    pub fn can_buffer(&self) -> bool {
        self.flush_pipeline || self.write_buf.can_buffer()
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
                Some((head, len)) => {
                    debug!("parsed {} headers ({} bytes)", head.headers.len(), len);
                    return Ok(Async::Ready(head))
                },
                None => {
                    if self.read_buf.capacity() >= self.max_buf_size {
                        debug!("max_buf_size ({}) reached, closing", self.max_buf_size);
                        return Err(::Error::TooLarge);
                    }
                },
            }
            match try_ready!(self.read_from_io()) {
                0 => {
                    trace!("parse eof");
                    return Err(::Error::Incomplete);
                }
                _ => {},
            }
        }
    }

    pub fn read_from_io(&mut self) -> Poll<usize, io::Error> {
        use bytes::BufMut;
        self.read_blocked = false;
        if self.read_buf.remaining_mut() < INIT_BUFFER_SIZE {
            self.read_buf.reserve(INIT_BUFFER_SIZE);
        }
        self.io.read_buf(&mut self.read_buf).map(|ok| {
            match ok {
                Async::Ready(n) => {
                    debug!("read {} bytes", n);
                    Async::Ready(n)
                },
                Async::NotReady => {
                    self.read_blocked = true;
                    Async::NotReady
                }
            }
        })
    }

    pub fn io_mut(&mut self) -> &mut T {
        &mut self.io
    }

    pub fn is_read_blocked(&self) -> bool {
        self.read_blocked
    }

    pub fn flush(&mut self) -> Poll<(), io::Error> {
        if self.flush_pipeline && !self.read_buf.is_empty() {
            //Ok(())
        } else if self.write_buf.remaining() == 0 {
            try_nb!(self.io.flush());
        } else {
            loop {
                let n = try_ready!(self.io.write_buf(&mut self.write_buf));
                debug!("flushed {} bytes", n);
                if self.write_buf.remaining() == 0 {
                    break;
                } else if n == 0 {
                    trace!("write returned zero, but {} bytes remaining", self.write_buf.remaining());
                    return Err(io::ErrorKind::WriteZero.into())
                }
            }
            try_nb!(self.io.flush())
        }
        Ok(Async::Ready(()))
    }
}

pub trait MemRead {
    fn read_mem(&mut self, len: usize) -> Poll<Bytes, io::Error>;
}

impl<T, B> MemRead for Buffered<T, B> 
where
    T: AsyncRead + AsyncWrite,
    B: Buf,
{
    fn read_mem(&mut self, len: usize) -> Poll<Bytes, io::Error> {
        if !self.read_buf.is_empty() {
            let n = ::std::cmp::min(len, self.read_buf.len());
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

    #[inline]
    pub fn buf(&self) -> &[u8] {
        &self.bytes.as_ref()[self.pos..]
    }

    #[inline]
    pub fn consume(&mut self, num: usize) {
        self.pos += num;
    }
}

impl Cursor<Vec<u8>> {
    fn maybe_reset(&mut self) {
        if self.pos != 0 && self.remaining() == 0 {
            self.pos = 0;
            unsafe {
                self.bytes.set_len(0);
            }
        }
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

impl<T: AsRef<[u8]>> Buf for Cursor<T> {
    #[inline]
    fn remaining(&self) -> usize {
        self.bytes.as_ref().len() - self.pos
    }

    #[inline]
    fn bytes(&self) -> &[u8] {
        self.buf()
    }

    #[inline]
    fn advance(&mut self, cnt: usize) {
        self.consume(cnt)
    }
}

// an internal buffer to collect writes before flushes
struct WriteBuf<B> {
    buf: BufDeque<B>,
    max_buf_size: usize,
    strategy: Strategy,
}

impl<B> WriteBuf<B> {
    fn new() -> WriteBuf<B> {
        WriteBuf {
            buf: BufDeque::new(),
            max_buf_size: DEFAULT_MAX_BUFFER_SIZE,
            strategy: Strategy::Queue,
        }
    }
}


impl<B> WriteBuf<B>
where
    B: Buf,
{
    fn set_strategy(&mut self, strategy: Strategy) {
        self.strategy = strategy;
    }

    fn buffer(&mut self, buf: B) {
        match self.strategy {
            Strategy::Flatten => {
                let head = self.head_mut();
                head.maybe_reset();
                head.bytes.put(buf);
            },
            Strategy::Queue => {
                self.buf.bufs.push_back(VecOrBuf::Buf(buf));
            },
        }
    }

    fn can_buffer(&self) -> bool {
        match self.strategy {
            Strategy::Flatten => {
                self.remaining() < self.max_buf_size
            },
            Strategy::Queue => {
                // for now, the simplest of heuristics
                self.buf.bufs.len() < MAX_BUF_LIST_BUFFERS
                    && self.remaining() < self.max_buf_size
            },
        }
    }

    fn head_mut(&mut self) -> &mut Cursor<Vec<u8>> {
        // this dance is brought to you, The Borrow Checker!

        let reuse_back = if let Some(&VecOrBuf::Vec(_)) = self.buf.bufs.back() {
            true
        } else {
            false
        };

        if !reuse_back {
            let head_buf = Cursor::new(Vec::with_capacity(INIT_BUFFER_SIZE));
            self.buf.bufs.push_back(VecOrBuf::Vec(head_buf));
        }
        if let Some(&mut VecOrBuf::Vec(ref mut v)) = self.buf.bufs.back_mut() {
            v
        } else {
            unreachable!("head_buf just pushed on back");
        }
    }
}

impl<B: Buf> fmt::Debug for WriteBuf<B> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("WriteBuf")
            .field("remaining", &self.remaining())
            .field("strategy", &self.strategy)
            .finish()
    }
}

impl<B: Buf> Buf for WriteBuf<B> {
    #[inline]
    fn remaining(&self) -> usize {
        self.buf.remaining()
    }

    #[inline]
    fn bytes(&self) -> &[u8] {
        self.buf.bytes()
    }

    #[inline]
    fn advance(&mut self, cnt: usize) {
        self.buf.advance(cnt)
    }

    #[inline]
    fn bytes_vec<'t>(&'t self, dst: &mut [&'t IoVec]) -> usize {
        self.buf.bytes_vec(dst)
    }
}

#[derive(Debug)]
enum Strategy {
    Flatten,
    Queue,
}

enum VecOrBuf<B> {
    Vec(Cursor<Vec<u8>>),
    Buf(B),
}

impl<B: Buf> Buf for VecOrBuf<B> {
    #[inline]
    fn remaining(&self) -> usize {
        match *self {
            VecOrBuf::Vec(ref v) => v.remaining(),
            VecOrBuf::Buf(ref b) => b.remaining(),
        }
    }

    #[inline]
    fn bytes(&self) -> &[u8] {
        match *self {
            VecOrBuf::Vec(ref v) => v.bytes(),
            VecOrBuf::Buf(ref b) => b.bytes(),
        }
    }

    #[inline]
    fn advance(&mut self, cnt: usize) {
        match *self {
            VecOrBuf::Vec(ref mut v) => v.advance(cnt),
            VecOrBuf::Buf(ref mut b) => b.advance(cnt),
        }
    }

    #[inline]
    fn bytes_vec<'t>(&'t self, dst: &mut [&'t IoVec]) -> usize {
        match *self {
            VecOrBuf::Vec(ref v) => {
                if v.has_remaining() {
                    v.bytes_vec(dst)
                } else {
                    0
                }
            },
            VecOrBuf::Buf(ref b) => {
                if b.has_remaining() {
                    b.bytes_vec(dst)
                } else {
                    0
                }
            },
        }
    }
}

struct BufDeque<T> {
    bufs: VecDeque<VecOrBuf<T>>,
}


impl<T> BufDeque<T> {
    fn new() -> BufDeque<T> {
        BufDeque {
            bufs: VecDeque::new(),
        }
    }
}

impl<T: Buf> Buf for BufDeque<T> {
    #[inline]
    fn remaining(&self) -> usize {
        self.bufs.iter()
            .map(|buf| buf.remaining())
            .sum()
    }

    #[inline]
    fn bytes(&self) -> &[u8] {
        for buf in &self.bufs {
            if buf.has_remaining() {
                return buf.bytes();
            }
        }
        &[]
    }

    #[inline]
    fn advance(&mut self, mut cnt: usize) {
        let mut maybe_reclaim = None;
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
            maybe_reclaim = self.bufs.pop_front();
        }

        if let Some(VecOrBuf::Vec(v)) = maybe_reclaim {
            trace!("reclaiming write buf Vec");
            self.bufs.push_back(VecOrBuf::Vec(v));
        }
    }

    #[inline]
    fn bytes_vec<'t>(&'t self, dst: &mut [&'t IoVec]) -> usize {
        if dst.is_empty() {
            return 0;
        }
        let mut vecs = 0;
        for buf in &self.bufs {
            vecs += buf.bytes_vec(&mut dst[vecs..]);
            if vecs == dst.len() {
                break;
            }
        }
        vecs
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

    let mut io_buf = Buffered::<_, Cursor<Vec<u8>>>::new(mock);

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
    let mut buffered = Buffered::<_, Cursor<Vec<u8>>>::new(mock);
    assert_eq!(buffered.parse::<::proto::ClientTransaction>().unwrap(), Async::NotReady);
    assert!(buffered.io.blocked());
}
