//! Client Connection Pooling
use std::borrow::ToOwned;
use std::collections::HashMap;
use std::fmt;
use std::io::{self, Read, Write};
use std::net::{SocketAddr, Shutdown};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};

use std::time::{Duration, Instant};

use net::{NetworkConnector, NetworkStream, DefaultConnector};
use client::scheme::Scheme;

/// The `NetworkConnector` that behaves as a connection pool used by hyper's `Client`.
pub struct Pool<C: NetworkConnector> {
    connector: C,
    inner: Arc<Mutex<PoolImpl<<C as NetworkConnector>::Stream>>>
}

/// Config options for the `Pool`.
#[derive(Debug)]
pub struct Config {
    /// The maximum idle connections *per host*.
    pub max_idle: usize,
}

impl Default for Config {
    #[inline]
    fn default() -> Config {
        Config {
            max_idle: 5,
        }
    }
}

// Because `Config` has all its properties public, it would be a breaking
// change to add new ones. Sigh.
#[derive(Debug)]
struct Config2 {
    idle_timeout: Option<Duration>,
    max_idle: usize,
}

#[derive(Debug)]
struct PoolImpl<S> {
    conns: HashMap<Key, Vec<PooledStreamInner<S>>>,
    config: Config2,
}

type Key = (String, u16, Scheme);

fn key<T: Into<Scheme>>(host: &str, port: u16, scheme: T) -> Key {
    (host.to_owned(), port, scheme.into())
}

impl Pool<DefaultConnector> {
    /// Creates a `Pool` with a `DefaultConnector`.
    #[inline]
    pub fn new(config: Config) -> Pool<DefaultConnector> {
        Pool::with_connector(config, DefaultConnector::default())
    }
}

impl<C: NetworkConnector> Pool<C> {
    /// Creates a `Pool` with a specified `NetworkConnector`.
    #[inline]
    pub fn with_connector(config: Config, connector: C) -> Pool<C> {
        Pool {
            connector: connector,
            inner: Arc::new(Mutex::new(PoolImpl {
                conns: HashMap::new(),
                config: Config2 {
                    idle_timeout: None,
                    max_idle: config.max_idle,
                }
            }))
        }
    }

    /// Set a duration for how long an idle connection is still valid.
    pub fn set_idle_timeout(&mut self, timeout: Option<Duration>) {
        self.inner.lock().unwrap().config.idle_timeout = timeout;
    }

    /// Clear all idle connections from the Pool, closing them.
    #[inline]
    pub fn clear_idle(&mut self) {
        self.inner.lock().unwrap().conns.clear();
    }
}

impl<S> PoolImpl<S> {
    fn reuse(&mut self, key: Key, conn: PooledStreamInner<S>) {
        trace!("reuse {:?}", key);
        let conns = self.conns.entry(key).or_insert(vec![]);
        if conns.len() < self.config.max_idle {
            conns.push(conn);
        }
    }
}

impl<C: NetworkConnector<Stream=S>, S: NetworkStream + Send> NetworkConnector for Pool<C> {
    type Stream = PooledStream<S>;
    fn connect(&self, host: &str, port: u16, scheme: &str) -> ::Result<PooledStream<S>> {
        let key = key(host, port, scheme);

        let inner = {
            // keep the mutex locked only in this block
            let mut locked = self.inner.lock().unwrap();
            let mut should_remove = false;
            let deadline = locked.config.idle_timeout.map(|dur| Instant::now() - dur);
            let inner = locked.conns.get_mut(&key).and_then(|vec| {
                while let Some(inner) = vec.pop() {
                    should_remove = vec.len() <= 1;
                    if let Some(deadline) = deadline {
                        if inner.idle.expect("idle is never missing inside pool") < deadline {
                            trace!("ejecting expired connection idle");
                            continue;
                        }
                    }
                    trace!("Pool had connection, using");
                    return Some(inner);
                }
                None
            });
            if should_remove {
                locked.conns.remove(&key);
            }
            inner
        };

        let inner = match inner {
            Some(inner) => inner,
            None => PooledStreamInner {
                key: key.clone(),
                idle: None,
                stream: try!(self.connector.connect(host, port, scheme)),
                previous_response_expected_no_content: false,
            }

        };
        Ok(PooledStream {
            has_read: false,
            inner: Some(inner),
            is_closed: AtomicBool::new(false),
            pool: self.inner.clone(),
        })
    }
}

/// A Stream that will try to be returned to the Pool when dropped.
pub struct PooledStream<S> {
    has_read: bool,
    inner: Option<PooledStreamInner<S>>,
    // mutated in &self methods
    is_closed: AtomicBool,
    pool: Arc<Mutex<PoolImpl<S>>>,
}

// manual impl to add the 'static bound for 1.7 compat
impl<S> fmt::Debug for PooledStream<S> where S: fmt::Debug + 'static {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("PooledStream")
           .field("inner", &self.inner)
           .field("has_read", &self.has_read)
           .field("is_closed", &self.is_closed.load(Ordering::Relaxed))
           .field("pool", &self.pool)
           .finish()
    }
}

impl<S: NetworkStream> PooledStream<S> {
    /// Take the wrapped stream out of the pool completely.
    pub fn into_inner(mut self) -> S {
        self.inner.take().expect("PooledStream lost its inner stream").stream
    }

    /// Gets a borrowed reference to the underlying stream.
    pub fn get_ref(&self) -> &S {
        &self.inner.as_ref().expect("PooledStream lost its inner stream").stream
    }

    #[cfg(test)]
    fn get_mut(&mut self) -> &mut S {
        &mut self.inner.as_mut().expect("PooledStream lost its inner stream").stream
    }
}

#[derive(Debug)]
struct PooledStreamInner<S> {
    key: Key,
    idle: Option<Instant>,
    stream: S,
    previous_response_expected_no_content: bool,
}

impl<S: NetworkStream> Read for PooledStream<S> {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut inner = self.inner.as_mut().unwrap();
        let n = try!(inner.stream.read(buf));
        if n == 0 {
            // if the wrapped stream returns EOF (Ok(0)), that means the
            // server has closed the stream. we must be sure this stream
            // is dropped and not put back into the pool.
            self.is_closed.store(true, Ordering::Relaxed);

            // if the stream has never read bytes before, then the pooled
            // stream may have been disconnected by the server while
            // we checked it back out
            if !self.has_read && inner.idle.is_some() {
                // idle being some means this is a reused stream
                Err(io::Error::new(
                    io::ErrorKind::ConnectionAborted,
                    "Pooled stream disconnected"
                ))
            } else {
                Ok(0)
            }
        } else {
            self.has_read = true;
            Ok(n)
        }
    }
}

impl<S: NetworkStream> Write for PooledStream<S> {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.as_mut().unwrap().stream.write(buf)
    }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        self.inner.as_mut().unwrap().stream.flush()
    }
}

impl<S: NetworkStream> NetworkStream for PooledStream<S> {
    #[inline]
    fn peer_addr(&mut self) -> io::Result<SocketAddr> {
        self.inner.as_mut().unwrap().stream.peer_addr()
            .map_err(|e| {
                self.is_closed.store(true, Ordering::Relaxed);
                e
            })
    }

    #[inline]
    fn set_read_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        self.inner.as_ref().unwrap().stream.set_read_timeout(dur)
            .map_err(|e| {
                self.is_closed.store(true, Ordering::Relaxed);
                e
            })
    }

    #[inline]
    fn set_write_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        self.inner.as_ref().unwrap().stream.set_write_timeout(dur)
            .map_err(|e| {
                self.is_closed.store(true, Ordering::Relaxed);
                e
            })
    }

    #[inline]
    fn close(&mut self, how: Shutdown) -> io::Result<()> {
        self.is_closed.store(true, Ordering::Relaxed);
        self.inner.as_mut().unwrap().stream.close(how)
    }

    #[inline]
    fn set_previous_response_expected_no_content(&mut self, expected: bool) {
        trace!("set_previous_response_expected_no_content {}", expected);
        self.inner.as_mut().unwrap().previous_response_expected_no_content = expected;
    }

    #[inline]
    fn previous_response_expected_no_content(&self) -> bool {
        let answer = self.inner.as_ref().unwrap().previous_response_expected_no_content;
        trace!("previous_response_expected_no_content {}", answer);
        answer
    }
}

impl<S> Drop for PooledStream<S> {
    fn drop(&mut self) {
        let is_closed = self.is_closed.load(Ordering::Relaxed);
        trace!("PooledStream.drop, is_closed={}", is_closed);
        if !is_closed {
            self.inner.take().map(|mut inner| {
                let now = Instant::now();
                inner.idle = Some(now);
                if let Ok(mut pool) = self.pool.lock() {
                    pool.reuse(inner.key.clone(), inner);
                }
                // else poisoned, give up
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use std::net::Shutdown;
    use std::io::Read;
    use std::time::Duration;
    use mock::{MockConnector};
    use net::{NetworkConnector, NetworkStream};

    use super::{Pool, key};

    macro_rules! mocked {
        () => ({
            Pool::with_connector(Default::default(), MockConnector)
        })
    }

    #[test]
    fn test_connect_and_drop() {
        let mut pool = mocked!();
        pool.set_idle_timeout(Some(Duration::from_millis(100)));
        let key = key("127.0.0.1", 3000, "http");
        let mut stream = pool.connect("127.0.0.1", 3000, "http").unwrap();
        assert_eq!(stream.get_ref().id, 0);
        stream.get_mut().id = 9;
        drop(stream);
        {
            let locked = pool.inner.lock().unwrap();
            assert_eq!(locked.conns.len(), 1);
            assert_eq!(locked.conns.get(&key).unwrap().len(), 1);
        }
        let stream = pool.connect("127.0.0.1", 3000, "http").unwrap(); //reused
        assert_eq!(stream.get_ref().id, 9);
        drop(stream);
        {
            let locked = pool.inner.lock().unwrap();
            assert_eq!(locked.conns.len(), 1);
            assert_eq!(locked.conns.get(&key).unwrap().len(), 1);
        }
    }

    #[test]
    fn test_closed() {
        let pool = mocked!();
        let mut stream = pool.connect("127.0.0.1", 3000, "http").unwrap();
        stream.close(Shutdown::Both).unwrap();
        drop(stream);
        let locked = pool.inner.lock().unwrap();
        assert_eq!(locked.conns.len(), 0);
    }

    #[test]
    fn test_eof_closes() {
        let pool = mocked!();

        let mut stream = pool.connect("127.0.0.1", 3000, "http").unwrap();
        assert_eq!(stream.read(&mut [0]).unwrap(), 0);
        drop(stream);
        let locked = pool.inner.lock().unwrap();
        assert_eq!(locked.conns.len(), 0);
    }

    #[test]
    fn test_idle_timeout() {
        let mut pool = mocked!();
        pool.set_idle_timeout(Some(Duration::from_millis(10)));
        let mut stream = pool.connect("127.0.0.1", 3000, "http").unwrap();
        assert_eq!(stream.get_ref().id, 0);
        stream.get_mut().id = 1337;
        drop(stream);
        ::std::thread::sleep(Duration::from_millis(100));
        let stream = pool.connect("127.0.0.1", 3000, "http").unwrap();
        assert_eq!(stream.get_ref().id, 0);
    }
}
