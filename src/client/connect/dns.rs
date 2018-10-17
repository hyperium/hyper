use std::{fmt, io, vec};
use std::net::{
    IpAddr, Ipv4Addr, Ipv6Addr,
    SocketAddr, ToSocketAddrs,
    SocketAddrV4, SocketAddrV6,
};
use std::sync::Arc;

use futures::{Async, Future, Poll};
use futures::future::{Executor, ExecuteError};
use futures::sync::oneshot;
use futures_cpupool::{Builder as CpuPoolBuilder};

use self::sealed::GaiTask;

/// Resolve a hostname to a set of IP addresses.
pub trait Resolve {
    /// The set of IP addresses to try to connect to.
    type Addrs: Iterator<Item=IpAddr>;
    /// A Future of the resolved set of addresses.
    type Future: Future<Item=Self::Addrs, Error=io::Error>;
    /// Resolve a hostname.
    fn resolve(&self, name: Name) -> Self::Future;
}

/// A domain name to resolve into IP addresses.
pub struct Name {
    host: String,
}

/// A resolver using blocking `getaddrinfo` calls in a threadpool.
#[derive(Clone)]
pub struct GaiResolver {
    executor: GaiExecutor,
}

pub struct GaiAddrs {
    inner: IpAddrs,
}

pub struct GaiFuture {
    rx: oneshot::SpawnHandle<IpAddrs, io::Error>,
}

impl Name {
    pub(super) fn new(host: String) -> Name {
        Name {
            host,
        }
    }

    /// View the hostname as a string slice.
    pub fn as_str(&self) -> &str {
        &self.host
    }
}

impl fmt::Debug for Name {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&self.host, f)
    }
}

impl GaiResolver {
    /// Construct a new `GaiResolver`.
    ///
    /// Takes number of DNS worker threads.
    pub fn new(threads: usize) -> Self {
        let pool = CpuPoolBuilder::new()
            .name_prefix("hyper-dns")
            .pool_size(threads)
            .create();
        GaiResolver::new_with_executor(pool)
    }

    /// Construct a new `GaiResolver` with a shared thread pool executor.
    ///
    /// Takes an executor to run blocking `getaddrinfo` tasks on.
    pub fn new_with_executor<E: 'static>(executor: E) -> Self
    where
        E: Executor<GaiTask> + Send + Sync,
    {
        GaiResolver {
            executor: GaiExecutor(Arc::new(executor)),
        }
    }
}

impl Resolve for GaiResolver {
    type Addrs = GaiAddrs;
    type Future = GaiFuture;

    fn resolve(&self, name: Name) -> Self::Future {
        let blocking = GaiBlocking::new(name.host);
        let rx = oneshot::spawn(blocking, &self.executor);
        GaiFuture {
            rx,
        }
    }
}

impl fmt::Debug for GaiResolver {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad("GaiResolver")
    }
}

impl Future for GaiFuture {
    type Item = GaiAddrs;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let addrs = try_ready!(self.rx.poll());
        Ok(Async::Ready(GaiAddrs {
            inner: addrs,
        }))
    }
}

impl fmt::Debug for GaiFuture {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad("GaiFuture")
    }
}

impl Iterator for GaiAddrs {
    type Item = IpAddr;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|sa| sa.ip())
    }
}

impl fmt::Debug for GaiAddrs {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad("GaiAddrs")
    }
}

#[derive(Clone)]
struct GaiExecutor(Arc<Executor<GaiTask> + Send + Sync>);

impl Executor<oneshot::Execute<GaiBlocking>> for GaiExecutor {
    fn execute(&self, future: oneshot::Execute<GaiBlocking>) -> Result<(), ExecuteError<oneshot::Execute<GaiBlocking>>> {
        self.0.execute(GaiTask { work: future })
            .map_err(|err| ExecuteError::new(err.kind(), err.into_future().work))
    }
}

pub(super) struct GaiBlocking {
    host: String,
}

impl GaiBlocking {
    pub(super) fn new(host: String) -> GaiBlocking {
        GaiBlocking { host }
    }
}

impl Future for GaiBlocking {
    type Item = IpAddrs;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        debug!("resolving host={:?}", self.host);
        (&*self.host, 0).to_socket_addrs()
            .map(|i| Async::Ready(IpAddrs { iter: i }))
    }
}

pub(super) struct IpAddrs {
    iter: vec::IntoIter<SocketAddr>,
}

impl IpAddrs {
    pub(super) fn new(addrs: Vec<SocketAddr>) -> Self {
        IpAddrs { iter: addrs.into_iter() }
    }

    pub(super) fn try_parse(host: &str, port: u16) -> Option<IpAddrs> {
        if let Ok(addr) = host.parse::<Ipv4Addr>() {
            let addr = SocketAddrV4::new(addr, port);
            return Some(IpAddrs { iter: vec![SocketAddr::V4(addr)].into_iter() })
        }
        if let Ok(addr) = host.parse::<Ipv6Addr>() {
            let addr = SocketAddrV6::new(addr, port, 0, 0);
            return Some(IpAddrs { iter: vec![SocketAddr::V6(addr)].into_iter() })
        }
        None
    }

    pub(super) fn split_by_preference(self) -> (IpAddrs, IpAddrs) {
        let preferring_v6 = self.iter
            .as_slice()
            .first()
            .map(SocketAddr::is_ipv6)
            .unwrap_or(false);

        let (preferred, fallback) = self.iter
            .partition::<Vec<_>, _>(|addr| addr.is_ipv6() == preferring_v6);

        (IpAddrs::new(preferred), IpAddrs::new(fallback))
    }

    pub(super) fn is_empty(&self) -> bool {
        self.iter.as_slice().is_empty()
    }
}

impl Iterator for IpAddrs {
    type Item = SocketAddr;
    #[inline]
    fn next(&mut self) -> Option<SocketAddr> {
        self.iter.next()
    }
}

// Make this Future unnameable outside of this crate.
pub(super) mod sealed {
    use super::*;
    // Blocking task to be executed on a thread pool.
    pub struct GaiTask {
        pub(super) work: oneshot::Execute<GaiBlocking>
    }

    impl fmt::Debug for GaiTask {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.pad("GaiTask")
        }
    }

    impl Future for GaiTask {
        type Item = ();
        type Error = ();

        fn poll(&mut self) -> Poll<(), ()> {
            self.work.poll()
        }
    }
}

#[cfg(test)]
mod tests {
    use std::net::{Ipv4Addr, Ipv6Addr};
    use super::*;

    #[test]
    fn test_ip_addrs_split_by_preference() {
        let v4_addr = (Ipv4Addr::new(127, 0, 0, 1), 80).into();
        let v6_addr = (Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1), 80).into();

        let (mut preferred, mut fallback) =
            IpAddrs { iter: vec![v4_addr, v6_addr].into_iter() }.split_by_preference();
        assert!(preferred.next().unwrap().is_ipv4());
        assert!(fallback.next().unwrap().is_ipv6());

        let (mut preferred, mut fallback) =
            IpAddrs { iter: vec![v6_addr, v4_addr].into_iter() }.split_by_preference();
        assert!(preferred.next().unwrap().is_ipv6());
        assert!(fallback.next().unwrap().is_ipv4());
    }
}
