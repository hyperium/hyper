use std::collections::HashMap;
use std::cmp;
use std::io::{self, Read, Write};
use std::sync::{Arc, Mutex};

use bytes::Buf;
use futures::{Async, Poll};
use futures::task::{self, Task};
use tokio_io::{AsyncRead, AsyncWrite};

use ::client::connect::{Connect, Connected, Destination};

#[derive(Debug)]
pub struct MockCursor {
    vec: Vec<u8>,
    pos: usize,
}

impl MockCursor {
    pub fn wrap(vec: Vec<u8>) -> MockCursor {
        MockCursor {
            vec: vec,
            pos: 0,
        }
    }
}

impl ::std::ops::Deref for MockCursor {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        &self.vec
    }
}

impl AsRef<[u8]> for MockCursor {
    fn as_ref(&self) -> &[u8] {
        &self.vec
    }
}

impl<S: AsRef<[u8]>> PartialEq<S> for MockCursor {
    fn eq(&self, other: &S) -> bool {
        self.vec == other.as_ref()
    }
}

impl Write for MockCursor {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        self.vec.extend(data);
        Ok(data.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Read for MockCursor {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        (&self.vec[self.pos..]).read(buf).map(|n| {
            self.pos += n;
            n
        })
    }
}

const READ_VECS_CNT: usize = 64;

#[derive(Debug)]
pub struct AsyncIo<T> {
    blocked: bool,
    bytes_until_block: usize,
    error: Option<io::Error>,
    flushed: bool,
    inner: T,
    max_read_vecs: usize,
    num_writes: usize,
    park_tasks: bool,
    task: Option<Task>,
}

impl<T> AsyncIo<T> {
    pub fn new(inner: T, bytes: usize) -> AsyncIo<T> {
        AsyncIo {
            blocked: false,
            bytes_until_block: bytes,
            error: None,
            flushed: false,
            inner: inner,
            max_read_vecs: READ_VECS_CNT,
            num_writes: 0,
            park_tasks: false,
            task: None,
        }
    }

    pub fn block_in(&mut self, bytes: usize) {
        self.bytes_until_block = bytes;

        if let Some(task) = self.task.take() {
            task.notify();
        }
    }

    pub fn error(&mut self, err: io::Error) {
        self.error = Some(err);
    }

    pub fn max_read_vecs(&mut self, cnt: usize) {
        assert!(cnt <= READ_VECS_CNT);
        self.max_read_vecs = cnt;
    }

    pub fn park_tasks(&mut self, enabled: bool) {
        self.park_tasks = enabled;
    }

    /*
    pub fn flushed(&self) -> bool {
        self.flushed
    }
    */

    pub fn blocked(&self) -> bool {
        self.blocked
    }

    pub fn num_writes(&self) -> usize {
        self.num_writes
    }

    fn would_block(&mut self) -> io::Error {
        self.blocked = true;
        if self.park_tasks {
            self.task = Some(task::current());
        }
        io::ErrorKind::WouldBlock.into()
    }

}

impl AsyncIo<MockCursor> {
    pub fn new_buf<T: Into<Vec<u8>>>(buf: T, bytes: usize) -> AsyncIo<MockCursor> {
        AsyncIo::new(MockCursor::wrap(buf.into()), bytes)
    }

    /*
    pub fn new_eof() -> AsyncIo<Buf> {
        AsyncIo::new(Buf::wrap(Vec::new().into()), 1)
    }
    */

    fn close(&mut self) {
        self.block_in(1);
        assert_eq!(self.inner.vec.len(), self.inner.pos);
        self.inner.vec.truncate(0);
        self.inner.pos = 0;
    }
}

impl<T: Read + Write> AsyncIo<T> {
    fn write_no_vecs<B: Buf>(&mut self, buf: &mut B) -> Poll<usize, io::Error> {
        if !buf.has_remaining() {
            return Ok(Async::Ready(0));
        }

        let n = try_nb!(self.write(buf.bytes()));
        buf.advance(n);
        Ok(Async::Ready(n))
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
            Err(self.would_block())
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
        self.num_writes += 1;
        if let Some(err) = self.error.take() {
            trace!("AsyncIo::write error");
            Err(err)
        } else if self.bytes_until_block == 0 {
            trace!("AsyncIo::write would block");
            Err(self.would_block())
        } else {
            trace!("AsyncIo::write; {} bytes", data.len());
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

    fn write_buf<B: Buf>(&mut self, buf: &mut B) -> Poll<usize, io::Error> {
        if self.max_read_vecs == 0 {
            return self.write_no_vecs(buf);
        }
        let r = {
            static DUMMY: &[u8] = &[0];
            let mut bufs = [From::from(DUMMY); READ_VECS_CNT];
            let i = Buf::bytes_vec(&buf, &mut bufs[..self.max_read_vecs]);
            let mut n = 0;
            let mut ret = Ok(0);
            // each call to write() will increase our count, but we assume
            // that if iovecs are used, its really only 1 write call.
            let num_writes = self.num_writes;
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
            self.num_writes = num_writes + 1;
            ret
        };
        match r {
            Ok(n) => {
                Buf::advance(buf, n);
                Ok(Async::Ready(n))
            }
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                Ok(Async::NotReady)
            }
            Err(e) => Err(e),
        }
    }
}

impl ::std::ops::Deref for AsyncIo<MockCursor> {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        &self.inner
    }
}

pub struct Duplex {
    inner: Arc<Mutex<DuplexInner>>,
}

struct DuplexInner {
    handle_read_task: Option<Task>,
    read: AsyncIo<MockCursor>,
    write: AsyncIo<MockCursor>,
}

impl Read for Duplex {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.lock().unwrap().read.read(buf)
    }
}

impl Write for Duplex {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut inner = self.inner.lock().unwrap();
        if let Some(task) = inner.handle_read_task.take() {
            trace!("waking DuplexHandle read");
            task.notify();
        }
        inner.write.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.lock().unwrap().write.flush()
    }
}

impl AsyncRead for Duplex {

}

impl AsyncWrite for Duplex {
    fn shutdown(&mut self) -> Poll<(), io::Error> {
        Ok(().into())
    }

    fn write_buf<B: Buf>(&mut self, buf: &mut B) -> Poll<usize, io::Error> {
        let mut inner = self.inner.lock().unwrap();
        if let Some(task) = inner.handle_read_task.take() {
            task.notify();
        }
        inner.write.write_buf(buf)
    }
}

pub struct DuplexHandle {
    inner: Arc<Mutex<DuplexInner>>,
}

impl DuplexHandle {
    pub fn read(&self, buf: &mut [u8]) -> Poll<usize, io::Error> {
        let mut inner = self.inner.lock().unwrap();
        assert!(buf.len() >= inner.write.inner.len());
        if inner.write.inner.is_empty() {
            trace!("DuplexHandle read parking");
            inner.handle_read_task = Some(task::current());
            return Ok(Async::NotReady);
        }
        inner.write.inner.vec.truncate(0);
        Ok(Async::Ready(inner.write.inner.len()))
    }

    pub fn write(&self, bytes: &[u8]) -> Poll<usize, io::Error> {
        let mut inner = self.inner.lock().unwrap();
        assert!(inner.read.inner.vec.is_empty());
        assert_eq!(inner.read.inner.pos, 0);
        inner
            .read
            .inner
            .vec
            .extend(bytes);
        inner.read.block_in(bytes.len());
        Ok(Async::Ready(bytes.len()))
    }
}

impl Drop for DuplexHandle {
    fn drop(&mut self) {
        trace!("mock duplex handle drop");
        let mut inner = self.inner.lock().unwrap();
        inner.read.close();
        inner.write.close();
    }
}

pub struct MockConnector {
    mocks: Mutex<HashMap<String, Vec<Duplex>>>,
}

impl MockConnector {
    pub fn new() -> MockConnector {
        MockConnector {
            mocks: Mutex::new(HashMap::new()),
        }
    }

    pub fn mock(&mut self, key: &str) -> DuplexHandle {
        let key = key.to_owned();
        let mut inner = DuplexInner {
            handle_read_task: None,
            read: AsyncIo::new_buf(Vec::new(), 0),
            write: AsyncIo::new_buf(Vec::new(), ::std::usize::MAX),
        };

        inner.read.park_tasks(true);
        inner.write.park_tasks(true);

        let inner = Arc::new(Mutex::new(inner));

        let duplex = Duplex {
            inner: inner.clone(),
        };
        let handle = DuplexHandle {
            inner: inner,
        };

        self.mocks.lock().unwrap().entry(key)
            .or_insert(Vec::new())
            .push(duplex);

        handle
    }
}

impl Connect for MockConnector {
    type Transport = Duplex;
    type Error = io::Error;
    type Future = ::futures::future::FutureResult<(Self::Transport, Connected), Self::Error>;

    fn connect(&self, dst: Destination) -> Self::Future {
        use futures::future;
        trace!("mock connect: {:?}", dst);
        let key = format!("{}://{}{}", dst.scheme(), dst.host(), if let Some(port) = dst.port() {
            format!(":{}", port)
        } else {
            "".to_owned()
        });
        let mut mocks = self.mocks.lock().unwrap();
        let mocks = mocks.get_mut(&key)
            .expect(&format!("unknown mocks uri: {}", key));
        assert!(!mocks.is_empty(), "no additional mocks for {}", key);
        future::ok((mocks.remove(0), Connected::new()))
    }
}
