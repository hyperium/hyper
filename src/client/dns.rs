use std::io;
use std::net::{
    Ipv4Addr, Ipv6Addr,
    SocketAddr, ToSocketAddrs,
    SocketAddrV4, SocketAddrV6,
};
use std::vec;

use futures::{Async, Future, Poll};
use futures::task;
use futures::future::lazy;
use futures::executor::Executor;
use futures::channel::oneshot;

pub struct Resolving {
    receiver: oneshot::Receiver<Result<IpAddrs, io::Error>>
}

impl Resolving {
    pub fn spawn(host: String, port: u16, executor: &mut Executor) -> Resolving {
        let (sender, receiver) = oneshot::channel();
        // The `Resolving` future will return an error when the sender is dropped,
        // so we can just ignore the spawn error here
        executor.spawn(Box::new(lazy(move |_| {
            debug!("resolving host={:?}, port={:?}", host, port);
            let result = (host.as_ref(), port).to_socket_addrs()
                .map(|i| IpAddrs { iter: i });
            sender.send(result).ok();
            Ok(())
        }))).ok();
        Resolving { receiver }
    }
}

impl Future for Resolving {
    type Item = IpAddrs;
    type Error = io::Error;

    fn poll(&mut self, cx: &mut task::Context) -> Poll<IpAddrs, io::Error> {
        match self.receiver.poll(cx) {
            Ok(Async::Pending) => Ok(Async::Pending),
            Ok(Async::Ready(Ok(ips))) => Ok(Async::Ready(ips)),
            Ok(Async::Ready(Err(err))) => Err(err),
            Err(_) =>
                Err(io::Error::new(io::ErrorKind::Other, "dns task was cancelled"))
        }
    }
}

pub struct IpAddrs {
    iter: vec::IntoIter<SocketAddr>,
}

impl IpAddrs {
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
}

impl Iterator for IpAddrs {
    type Item = SocketAddr;
    #[inline]
    fn next(&mut self) -> Option<SocketAddr> {
        self.iter.next()
    }
}
