use std::io;
use std::net::{SocketAddr, ToSocketAddrs};
use std::vec;

use ::futures::{Async, Future, Poll};

pub struct Work {
    host: String,
    port: u16
}

impl Work {
    pub fn new(host: String, port: u16) -> Work {
        Work { host: host, port: port }
    }
}

impl Future for Work {
    type Item = IpAddrs;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        debug!("resolve host={:?}, port={:?}", self.host, self.port);
        (&*self.host, self.port).to_socket_addrs()
            .map(|i| Async::Ready(IpAddrs { iter: i }))
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
