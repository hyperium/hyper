use std::io;
use std::net::{
    Ipv4Addr, Ipv6Addr,
    SocketAddr, ToSocketAddrs,
    SocketAddrV4, SocketAddrV6,
};
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
        debug!("resolving host={:?}, port={:?}", self.host, self.port);
        (&*self.host, self.port).to_socket_addrs()
            .map(|i| Async::Ready(IpAddrs { iter: i }))
    }
}

pub struct IpAddrs {
    iter: vec::IntoIter<SocketAddr>,
}

impl IpAddrs {
    pub fn new(addrs: Vec<SocketAddr>) -> Self {
        IpAddrs { iter: addrs.into_iter() }
    }

    pub fn try_parse(host: &str, port: u16) -> Option<IpAddrs> {
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

    pub fn split_by_preference(self) -> (IpAddrs, IpAddrs) {
        let preferring_v6 = self.iter
            .as_slice()
            .first()
            .map(SocketAddr::is_ipv6)
            .unwrap_or(false);

        let (preferred, fallback) = self.iter
            .partition::<Vec<_>, _>(|addr| addr.is_ipv6() == preferring_v6);

        (IpAddrs::new(preferred), IpAddrs::new(fallback))
    }

    pub fn is_empty(&self) -> bool {
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
