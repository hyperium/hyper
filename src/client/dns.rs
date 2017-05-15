use std::fmt;
use std::io;
use std::net::{SocketAddr, ToSocketAddrs};
use std::vec;

use ::futures::{Future, Poll};
use ::futures_cpupool::{CpuPool, CpuFuture};

/// Trait for resolving a host and port into a set of ip addresses.
pub trait DnsService {
    /// Response representing a set of ip addresses.
    type Response: IntoIterator<Item=SocketAddr>;
    /// Future being returned from `resolve`.
    type Future: Future<Item=Self::Response, Error=io::Error>;

    /// Resolves a host and port into a set of ip addresses.
    fn resolve(&self, host: String, port: u16) -> Self::Future;
}

#[derive(Clone)]
/// An implementation of a dns service using a thread pool.
pub struct DnsPool {
    pool: CpuPool,
}

impl DnsPool {
    /// Create a new dns pool given a number of worker threads.
    pub fn new(threads: usize) -> DnsPool {
        DnsPool {
            pool: CpuPool::new(threads)
        }
    }
}

impl fmt::Debug for DnsPool {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad("DnsPool")
    }
}

impl DnsService for DnsPool {
    type Response = IpAddrs;
    type Future = DnsQuery;
    fn resolve(&self, host: String, port: u16) -> Self::Future {
        DnsQuery(self.pool.spawn_fn(move || work(host, port)))
    }
}

/// A `Future` that will resolve to `IpAddrs`.
pub struct DnsQuery(CpuFuture<IpAddrs, io::Error>);

impl fmt::Debug for DnsQuery {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad("DnsQuery")
    }
}

impl Future for DnsQuery {
    type Item = IpAddrs;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        self.0.poll()
    }
}

/// A set of ip addresses.
pub struct IpAddrs {
    iter: vec::IntoIter<SocketAddr>,
}

impl fmt::Debug for IpAddrs {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad("IpAddrs")
    }
}

impl Iterator for IpAddrs {
    type Item = SocketAddr;
    #[inline]
    fn next(&mut self) -> Option<SocketAddr> {
        self.iter.next()
    }
}

type Answer = io::Result<IpAddrs>;

fn work(hostname: String, port: u16) -> Answer {
    debug!("resolve {:?}:{:?}", hostname, port);
    (&*hostname, port).to_socket_addrs().map(|i| IpAddrs { iter: i })
}
