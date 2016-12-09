use std::collections::hash_map::{HashMap, Entry};
use std::hash::Hash;
use std::fmt;
use std::io::{self, Read, Write};
use std::net::SocketAddr;

use futures::{Future, Poll, Async};
use tokio::io::Io;
use tokio::reactor::Handle;
use tokio::net::{TcpStream, TcpStreamNew};
use tokio_tls as tls;
use tokio_service::Service;
use url::Url;

use super::dns;

pub type DefaultConnector = HttpsConnector;
pub type HttpsStream = tls::TlsStream<TcpStream>;

/*
/// A connector creates a Transport to a remote address..
pub trait Connect {
    /// Type of Transport to create
    type Output: Transport;
    /// dox
    type Fut: Future<Item=Self::Output, Error=io::Error>;
    /// The key used to determine if an existing socket can be used.
    type Key: Eq + Hash + Clone + fmt::Debug;
    /// Returns the key based off the Url.
    fn key(&self, &Url) -> Option<Self::Key>;
    /// Connect to a remote address.
    fn connect(&mut self, &Url) -> Self::Fut;
}
*/

type Scheme = String;
type Port = u16;

/// A connector for the `http` scheme.
#[derive(Clone)]
pub struct HttpConnector {
    dns: dns::Dns,
    handle: Handle,
}

impl HttpConnector {

    /// Construct a new HttpConnector.
    ///
    /// Takes number of DNS worker threads.
    pub fn new(handle: &Handle, threads: usize) -> HttpConnector {
        HttpConnector {
            dns: dns::Dns::new(threads),
            handle: handle.clone(),
        }
    }
}

/*
impl Default for HttpConnector {
    fn default() -> HttpConnector {
        HttpConnector::new(4)
    }
}
*/

impl fmt::Debug for HttpConnector {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("HttpConnector")
            .finish()
    }
}

impl Service for HttpConnector {
    type Request = Url;
    type Response = TcpStream;
    type Error = io::Error;
    type Future = Connecting;

    fn call(&self, url: Url) -> Self::Future {
        debug!("Http::connect({:?})", url);
        let host = url.host_str().expect("http scheme must have a host");
        let port = url.port_or_known_default().unwrap_or(80);

        Connecting {
            state: State::Resolving(self.dns.resolve(host.into(), port)),
            handle: self.handle.clone(),
        }
    }

}

pub struct HttpsConnector {
    dns: dns::Dns,
    handle: Handle,
}

impl HttpsConnector {

    /// Construct a new HttpsConnector.
    ///
    /// Takes number of DNS worker threads.
    pub fn new(handle: &Handle, threads: usize) -> HttpsConnector {
        HttpsConnector {
            dns: dns::Dns::new(threads),
            handle: handle.clone(),
        }
    }
}

impl fmt::Debug for HttpsConnector {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("HttpsConnector")
            .finish()
    }
}

impl Service for HttpsConnector {
    type Request = Url;
    type Response = MaybeHttpsStream;
    type Error = io::Error;
    type Future = TlsConnecting;

    fn call(&self, url: Url) -> Self::Future {
        debug!("Https::connect({:?})", url);
        let is_https = url.scheme() == "https";
        let host = url.host_str().expect("http scheme must have a host");
        let port = url.port_or_known_default().unwrap_or(80);

        let host = host.to_owned();

        let connecting = Connecting {
            state: State::Resolving(self.dns.resolve(host.clone(), port)),
            handle: self.handle.clone(),
        };
        if is_https {
            Box::new(connecting.and_then(move |tcp| {
                let ctx = tls::ClientContext::new().unwrap();
                ctx.handshake(&host, tcp)
            }).map(|tls| MaybeHttpsStream::Https(tls)))
        } else {
            Box::new(connecting.map(|tcp| MaybeHttpsStream::Http(tcp)))
        }
    }

}

pub type TlsConnecting = Box<Future<Item=MaybeHttpsStream, Error=io::Error>>;

pub struct Connecting {
    state: State,
    handle: Handle,
}

enum State {
    Resolving(dns::Query),
    Connecting(ConnectingTcp),
    Error(Option<io::Error>)
}

impl Future for Connecting {
    type Item = TcpStream;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        loop {
            let state;
            match self.state {
                State::Resolving(ref mut query) => {
                    match try!(query.poll()) {
                        Async::NotReady => return Ok(Async::NotReady),
                        Async::Ready(addrs) => {
                            state = State::Connecting(ConnectingTcp {
                                addrs: addrs,
                                current: None,
                            })
                        }
                    };
                },
                State::Connecting(ref mut c) => return c.poll(&self.handle).map_err(From::from),
                State::Error(ref mut err) => return Err(err.take().expect("polled Connecting too many times")),
            }
            self.state = state;
        }
    }
}

struct ConnectingTcp {
    addrs: dns::IpAddrs,
    current: Option<TcpStreamNew>,
}

impl ConnectingTcp {
    // not a Future, since passing a &Handle to poll
    fn poll(&mut self, handle: &Handle) -> Poll<TcpStream, io::Error> {
        let mut err = None;
        loop {
            if let Some(ref mut current) = self.current {
                match current.poll() {
                    Ok(ok) => return Ok(ok),
                    Err(e) => {
                        trace!("connect error {:?}", e);
                        err = Some(e);
                        if let Some(addr) = self.addrs.next() {
                            debug!("connecting to {:?}", addr);
                            *current = TcpStream::connect(&addr, handle);
                            continue;
                        }
                    }
                }
            } else if let Some(addr) = self.addrs.next() {
                debug!("connecting to {:?}", addr);
                self.current = Some(TcpStream::connect(&addr, handle));
                continue;
            }

            return Err(err.take().expect("missing connect error"));
        }
    }
}

pub enum MaybeHttpsStream {
    Http(TcpStream),
    Https(HttpsStream),
}

impl Read for MaybeHttpsStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match *self {
            MaybeHttpsStream::Http(ref mut s) => s.read(buf),
            MaybeHttpsStream::Https(ref mut s) => s.read(buf),
        }
    }
}

impl Write for MaybeHttpsStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match *self {
            MaybeHttpsStream::Http(ref mut s) => s.write(buf),
            MaybeHttpsStream::Https(ref mut s) => s.write(buf),
        }
    }


    fn flush(&mut self) -> io::Result<()> {
        match *self {
            MaybeHttpsStream::Http(ref mut s) => s.flush(),
            MaybeHttpsStream::Https(ref mut s) => s.flush(),
        }
    }
}

impl Io for MaybeHttpsStream {
    fn poll_read(&mut self) -> Async<()> {
        match *self {
            MaybeHttpsStream::Http(ref mut s) => s.poll_read(),
            MaybeHttpsStream::Https(ref mut s) => s.poll_read(),
        }
    }

    fn poll_write(&mut self) -> Async<()> {
        match *self {
            MaybeHttpsStream::Http(ref mut s) => s.poll_write(),
            MaybeHttpsStream::Https(ref mut s) => s.poll_write(),
        }
    }
}

/*
/// A connector that can protect HTTP streams using SSL.
#[derive(Debug, Default)]
pub struct HttpsConnector<S: SslClient> {
    http: HttpConnector,
    ssl: S
}

impl<S: SslClient> HttpsConnector<S> {
    /// Create a new connector using the provided SSL implementation.
    pub fn new(s: S) -> HttpsConnector<S> {
        HttpsConnector {
            http: HttpConnector::default(),
            ssl: s,
        }
    }
}
*/
