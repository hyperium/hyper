use std::io;
use std::net::{SocketAddr, ToSocketAddrs};
use std::vec;

use ::futures::{Future, Poll};
use ::futures_cpupool::{CpuPool, CpuFuture};

#[derive(Clone)]
pub struct Dns {
    pool: CpuPool,
}

impl Dns {
    pub fn new(threads: usize) -> Dns {
        Dns {
            pool: CpuPool::new(threads)
        }
    }

    pub fn resolve(&self, host: String, port: u16) -> Query {
        Query(self.pool.spawn_fn(move || work(host, port)))
    }
}

pub struct Query(CpuFuture<IpAddrs, io::Error>);

impl Future for Query {
    type Item = IpAddrs;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        self.0.poll()
    }
}

pub struct IpAddrs {
    iter: vec::IntoIter<SocketAddr>,
}

impl Iterator for IpAddrs {
    type Item = SocketAddr;
    #[inline]
    fn next(&mut self) -> Option<SocketAddr> {
        self.iter.next()
    }
}

pub type Answer = io::Result<IpAddrs>;

fn work(hostname: String, port: u16) -> Answer {
    debug!("resolve {:?}:{:?}", hostname, port);
    (&*hostname, port).to_socket_addrs().map(|i| IpAddrs { iter: i })
}
