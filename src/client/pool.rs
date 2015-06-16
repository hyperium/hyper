//! Client Connection Pooling
use std::borrow::ToOwned;
use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::net::{SocketAddr, Shutdown};
use std::sync::{Arc, Mutex};

#[cfg(feature = "timeouts")]
use std::time::Duration;

use net::{NetworkConnector, NetworkStream, DefaultConnector};

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

#[derive(Debug)]
struct PoolImpl<S> {
    conns: HashMap<Key, Vec<S>>,
    config: Config,
}

type Key = (String, u16, Scheme);

fn key<T: Into<Scheme>>(host: &str, port: u16, scheme: T) -> Key {
    (host.to_owned(), port, scheme.into())
}

#[derive(Clone, PartialEq, Eq, Debug, Hash)]
enum Scheme {
    Http,
    Https,
    Other(String)
}

impl<'a> From<&'a str> for Scheme {
    fn from(s: &'a str) -> Scheme {
        match s {
            "http" => Scheme::Http,
            "https" => Scheme::Https,
            s => Scheme::Other(String::from(s))
        }
    }
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
                config: config,
            }))
        }
    }

    /// Clear all idle connections from the Pool, closing them.
    #[inline]
    pub fn clear_idle(&mut self) {
        self.inner.lock().unwrap().conns.clear();
    }
}

impl<S> PoolImpl<S> {
    fn reuse(&mut self, key: Key, conn: S) {
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
        let mut locked = self.inner.lock().unwrap();
        let mut should_remove = false;
        let conn = match locked.conns.get_mut(&key) {
            Some(ref mut vec) => {
                trace!("Pool had connection, using");
                should_remove = vec.len() == 1;
                vec.pop().unwrap()
            }
            _ => try!(self.connector.connect(host, port, scheme))
        };
        if should_remove {
            locked.conns.remove(&key);
        }
        Ok(PooledStream {
            inner: Some((key, conn)),
            is_closed: false,
            pool: self.inner.clone()
        })
    }
}

/// A Stream that will try to be returned to the Pool when dropped.
pub struct PooledStream<S> {
    inner: Option<(Key, S)>,
    is_closed: bool,
    pool: Arc<Mutex<PoolImpl<S>>>
}

impl<S: NetworkStream> Read for PooledStream<S> {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.as_mut().unwrap().1.read(buf)
    }
}

impl<S: NetworkStream> Write for PooledStream<S> {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.as_mut().unwrap().1.write(buf)
    }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        self.inner.as_mut().unwrap().1.flush()
    }
}

impl<S: NetworkStream> NetworkStream for PooledStream<S> {
    #[inline]
    fn peer_addr(&mut self) -> io::Result<SocketAddr> {
        self.inner.as_mut().unwrap().1.peer_addr()
    }

    #[cfg(feature = "timeouts")]
    #[inline]
    fn set_read_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        self.inner.as_ref().unwrap().1.set_read_timeout(dur)
    }

    #[cfg(feature = "timeouts")]
    #[inline]
    fn set_write_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        self.inner.as_ref().unwrap().1.set_write_timeout(dur)
    }

    #[inline]
    fn close(&mut self, how: Shutdown) -> io::Result<()> {
        self.is_closed = true;
        self.inner.as_mut().unwrap().1.close(how)
    }
}

impl<S> Drop for PooledStream<S> {
    fn drop(&mut self) {
        trace!("PooledStream.drop, is_closed={}", self.is_closed);
        if !self.is_closed {
            self.inner.take().map(|(key, conn)| {
                if let Ok(mut pool) = self.pool.lock() {
                    pool.reuse(key, conn);
                }
                // else poisoned, give up
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use std::net::Shutdown;
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
        let pool = mocked!();
        let key = key("127.0.0.1", 3000, "http");
        pool.connect("127.0.0.1", 3000, "http").unwrap();
        {
            let locked = pool.inner.lock().unwrap();
            assert_eq!(locked.conns.len(), 1);
            assert_eq!(locked.conns.get(&key).unwrap().len(), 1);
        }
        pool.connect("127.0.0.1", 3000, "http").unwrap(); //reused
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
}
