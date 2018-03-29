use std::collections::HashMap;
use std::cmp;
use std::io::{self, Read, Write};
use std::sync::{Arc, Mutex};

use futures::{Async, Poll};
use futures::task;
use futures::io::{AsyncRead, AsyncWrite};
use iovec::IoVec;

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
    task: Option<task::Waker>,
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
            task.wake();
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

    fn would_block<X, E>(&mut self, cx: &mut task::Context) -> Poll<X, E> {
        self.blocked = true;
        if self.park_tasks {
            self.task = Some(cx.waker().clone());
        }
        Ok(Async::Pending)
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

impl<S: AsRef<[u8]>, T: AsRef<[u8]>> PartialEq<S> for AsyncIo<T> {
    fn eq(&self, other: &S) -> bool {
        self.inner.as_ref() == other.as_ref()
    }
}

impl<T: Read> AsyncRead for AsyncIo<T> {
    fn poll_read(&mut self, cx: &mut task::Context, buf: &mut [u8]) -> Poll<usize, io::Error> {
        self.blocked = false;
        if let Some(err) = self.error.take() {
            Err(err)
        } else if self.bytes_until_block == 0 {
            self.would_block(cx)
        } else {
            let n = cmp::min(self.bytes_until_block, buf.len());
            let n = try!(self.inner.read(&mut buf[..n]));
            self.bytes_until_block -= n;
            Ok(Async::Ready(n))
        }
    }
}

impl<T: Read + Write> AsyncIo<T> {
    fn write_no_vecs(&mut self, cx: &mut task::Context, buf: &[u8]) -> Poll<usize, io::Error> {
        if buf.len() == 0 {
            return Ok(Async::Ready(0));
        }

        self.poll_write(cx, buf)
    }
}

impl<T: Read + Write> AsyncWrite for AsyncIo<T> {
    fn poll_write(&mut self, cx: &mut task::Context, buf: &[u8]) -> Poll<usize, io::Error> {
        self.num_writes += 1;
        if let Some(err) = self.error.take() {
            trace!("AsyncIo::write error");
            Err(err)
        } else if self.bytes_until_block == 0 {
            trace!("AsyncIo::write would block");
            self.would_block(cx)
        } else {
            trace!("AsyncIo::write; {} bytes", buf.len());
            self.flushed = false;
            let n = cmp::min(self.bytes_until_block, buf.len());
            let n = try!(self.inner.write(&buf[..n]));
            self.bytes_until_block -= n;
            Ok(Async::Ready(n))
        }
    }

    fn poll_flush(&mut self, _cx: &mut task::Context) -> Poll<(), io::Error> {
        self.flushed = true;
        try!(self.inner.flush());
        Ok(Async::Ready(()))
    }

    fn poll_close(&mut self, _cx: &mut task::Context) -> Poll<(), io::Error> {
        Ok(().into())
    }

    fn poll_vectored_write(&mut self, cx: &mut task::Context, vec: &[&IoVec]) -> Poll<usize, io::Error> {
        if self.max_read_vecs == 0 {
            if let Some(ref first_iovec) = vec.get(0) {
                return self.write_no_vecs(cx, &*first_iovec)
            } else {
                return Ok(Async::Ready(0));
            }
        }

        let mut n = 0;
        let mut ret = Ok(Async::Ready(0));
        // each call to poll_write() will increase our count, but we assume
        // that if iovecs are used, its really only 1 write call.
        let num_writes = self.num_writes;
        for buf in vec {
            match self.poll_write(cx, &buf) {
                Ok(Async::Ready(num)) => {
                    n += num;
                    ret = Ok(Async::Ready(n));
                },
                Ok(Async::Pending) => {
                    if let Ok(Async::Ready(0)) = ret {
                        ret = Ok(Async::Pending);
                    }
                    break;
                },
                Err(err) => {
                    ret = Err(err);
                    break;
                }
            }
        }
        self.num_writes = num_writes + 1;
        ret
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
    handle_read_task: Option<task::Waker>,
    read: AsyncIo<MockCursor>,
    write: AsyncIo<MockCursor>,
}

impl AsyncRead for Duplex {
    fn poll_read(&mut self, cx: &mut task::Context, buf: &mut [u8]) -> Poll<usize, io::Error> {
        self.inner.lock().unwrap().read.poll_read(cx, buf)
    }
}

impl AsyncWrite for Duplex {
    fn poll_write(&mut self, cx: &mut task::Context, buf: &[u8]) -> Poll<usize, io::Error> {
        let mut inner = self.inner.lock().unwrap();
        if let Some(task) = inner.handle_read_task.take() {
            trace!("waking DuplexHandle read");
            task.wake();
        }
        inner.write.poll_write(cx, buf)
    }

    fn poll_flush(&mut self, cx: &mut task::Context) -> Poll<(), io::Error> {
        self.inner.lock().unwrap().write.poll_flush(cx)
    }

    fn poll_close(&mut self, _cx: &mut task::Context) -> Poll<(), io::Error> {
        Ok(().into())
    }
}

pub struct DuplexHandle {
    inner: Arc<Mutex<DuplexInner>>,
}

impl DuplexHandle {
    pub fn read(&self, cx: &mut task::Context, buf: &mut [u8]) -> Poll<usize, io::Error> {
        let mut inner = self.inner.lock().unwrap();
        assert!(buf.len() >= inner.write.inner.len());
        if inner.write.inner.is_empty() {
            trace!("DuplexHandle read parking");
            inner.handle_read_task = Some(cx.waker().clone());
            return Ok(Async::Pending);
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
