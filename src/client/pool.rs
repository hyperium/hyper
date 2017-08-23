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

use self::stale::{StaleCheck, Stale};

/// The `NetworkConnector` that behaves as a connection pool used by hyper's `Client`.
pub struct Pool<C: NetworkConnector> {
    connector: C,
    inner: Arc<Mutex<PoolImpl<<C as NetworkConnector>::Stream>>>,
    stale_check: Option<StaleCallback<C::Stream>>,
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
            })),
            stale_check: None,
        }
    }

    /// Set a duration for how long an idle connection is still valid.
    pub fn set_idle_timeout(&mut self, timeout: Option<Duration>) {
        self.inner.lock().unwrap().config.idle_timeout = timeout;
    }

    pub fn set_stale_check<F>(&mut self, callback: F)
    where F: Fn(StaleCheck<C::Stream>) -> Stale + Send + Sync + 'static {
        self.stale_check = Some(Box::new(callback));
    }

    /// Clear all idle connections from the Pool, closing them.
    #[inline]
    pub fn clear_idle(&mut self) {
        self.inner.lock().unwrap().conns.clear();
    }

    // private

    fn checkout(&self, key: &Key) -> Option<PooledStreamInner<C::Stream>> {
        while let Some(mut inner) = self.lookup(key) {
            if let Some(ref stale_check) = self.stale_check {
                let dur = inner.idle.expect("idle is never missing inside pool").elapsed();
                let arg = stale::check(&mut inner.stream, dur);
                if stale_check(arg).is_stale() {
                    trace!("ejecting stale connection");
                    continue;
                }
            }
            return Some(inner);
        }
        None
    }


    fn lookup(&self, key: &Key) -> Option<PooledStreamInner<C::Stream>> {
        let mut locked = self.inner.lock().unwrap();
        let mut should_remove = false;
        let deadline = locked.config.idle_timeout.map(|dur| Instant::now() - dur);
        let inner = locked.conns.get_mut(key).and_then(|vec| {
            while let Some(inner) = vec.pop() {
                should_remove = vec.is_empty();
                if let Some(deadline) = deadline {
                    if inner.idle.expect("idle is never missing inside pool") < deadline {
                        trace!("ejecting expired connection");
                        continue;
                    }
                }
                return Some(inner);
            }
            None
        });
        if should_remove {
            locked.conns.remove(key);
        }
        inner
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
        let inner = match self.checkout(&key) {
            Some(inner) => {
                trace!("Pool had connection, using");
                inner
            },
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

type StaleCallback<S> = Box<Fn(StaleCheck<S>) -> Stale + Send + Sync + 'static>;

// private on purpose
//
// Yes, I know! Shame on me! This hurts docs! And it means it only
// works with closures! I know!
//
// The thing is, this is experiemental. I'm not certain about the naming.
// Or other things. So I don't really want it in the docs, yet.
//
// As for only working with closures, that's fine. A closure is probably
// enough, and if it isn't, well you can grab the stream and duration and
// pass those to a function, and then figure out whether to call stale()
// or fresh() based on the return value.
//
// Point is, it's not that bad. And it's not ready to publicize.
mod stale {
    use std::time::Duration;

    pub struct StaleCheck<'a, S: 'a> {
        stream: &'a mut S,
        duration: Duration,
    }

    #[inline]
    pub fn check<'a, S: 'a>(stream: &'a mut S, dur: Duration) -> StaleCheck<'a, S> {
        StaleCheck {
            stream: stream,
            duration: dur,
        }
    }

    impl<'a, S: 'a> StaleCheck<'a, S> {
        pub fn stream(&mut self) -> &mut S {
            self.stream
        }

        pub fn idle_duration(&self) -> Duration {
            self.duration
        }

        pub fn stale(self) -> Stale {
            Stale(true)
        }

        pub fn fresh(self) -> Stale {
            Stale(false)
        }
    }

    pub struct Stale(bool);


    impl Stale {
        #[inline]
        pub fn is_stale(self) -> bool {
            self.0
        }
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
        let inner = self.inner.as_mut().unwrap();
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
    fn test_double_connect_reuse() {
        let mut pool = mocked!();
        pool.set_idle_timeout(Some(Duration::from_millis(100)));
        let key = key("127.0.0.1", 3000, "http");
        let stream1 = pool.connect("127.0.0.1", 3000, "http").unwrap();
        let stream2 = pool.connect("127.0.0.1", 3000, "http").unwrap();
        drop(stream1);
        drop(stream2);
        let stream1 = pool.connect("127.0.0.1", 3000, "http").unwrap();
        {
            let locked = pool.inner.lock().unwrap();
            assert_eq!(locked.conns.len(), 1);
            assert_eq!(locked.conns.get(&key).unwrap().len(), 1);
        }
        let _ = stream1;
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
    fn test_read_conn_aborted() {
        let pool = mocked!();

        pool.connect("127.0.0.1", 3000, "http").unwrap();
        let mut stream = pool.connect("127.0.0.1", 3000, "http").unwrap();
        let err = stream.read(&mut [0]).unwrap_err();
        assert_eq!(err.kind(), ::std::io::ErrorKind::ConnectionAborted);
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
