use std::fmt;
use std::io::{self, Read, Write};
//use std::net::SocketAddr;

use futures::{Future, Poll, Async};
use native_tls::TlsConnector;
use tokio::io::Io;
use tokio::reactor::Handle;
use tokio::net::{TcpStream, TcpStreamNew};
use tokio_service::Service;
use tokio_tls::{TlsStream, TlsConnectorExt};
use url::Url;

use super::dns;

/// The default connector used by the Client.
pub type DefaultConnector = HttpsConnector;
/// A TCP stream protected by TLS.
pub type HttpsStream = TlsStream<TcpStream>;

/// A connector creates an Io to a remote address..
///
/// This trait is not implemented directly, and only exists to make
/// the intent clearer. A connector should implement `Service` with
/// `Request=Url` and `Response: Io` instead.
pub trait Connect: Service<Request=Url, Error=io::Error> + 'static {
    /// The connected Io Stream.
    type Output: Io + 'static;
    /// A Future that will resolve to the connected Stream.
    type Future: Future<Item=Self::Output, Error=io::Error> + 'static;
    /// Connect to a remote address.
    fn connect(&mut self, Url) -> <Self as Connect>::Future;
}

impl<T> Connect for T
where T: Service<Request=Url, Error=io::Error> + 'static,
      T::Response: Io,
      T::Future: Future<Error=io::Error>,
{
    type Output = T::Response;
    type Future = T::Future;

    fn connect(&mut self, url: Url) -> <Self as Connect>::Future {
        self.call(url)
    }
}

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
    pub fn new(threads: usize, handle: &Handle) -> HttpConnector {
        HttpConnector {
            dns: dns::Dns::new(threads),
            handle: handle.clone(),
        }
    }
}

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
    type Future = HttpConnecting;

    fn call(&mut self, url: Url) -> Self::Future {
        debug!("Http::connect({:?})", url);
        let host = match url.host_str() {
            Some(s) => s,
            None => return HttpConnecting {
                state: State::Error(Some(io::Error::new(io::ErrorKind::InvalidInput, "invalid url"))),
                handle: self.handle.clone(),
            },
        };
        let port = url.port_or_known_default().unwrap_or(80);

        HttpConnecting {
            state: State::Resolving(self.dns.resolve(host.into(), port)),
            handle: self.handle.clone(),
        }
    }

}

/// A Connector for the `https` scheme.
#[derive(Clone)]
pub struct HttpsConnector {
    dns: dns::Dns,
    handle: Handle,
}

impl HttpsConnector {

    /// Construct a new HttpsConnector.
    ///
    /// Takes number of DNS worker threads.
    pub fn new(threads: usize, handle: &Handle) -> HttpsConnector {
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
    type Future = HttpsConnecting;

    fn call(&mut self, url: Url) -> Self::Future {
        debug!("Https::connect({:?})", url);
        let is_https = url.scheme() == "https";
        let host = match url.host_str() {
            Some(s) => s,
            None => return HttpsConnecting(Box::new(::futures::future::err(io::Error::new(io::ErrorKind::InvalidInput, "invalid url")))),
        };
        let port = url.port_or_known_default().unwrap_or(80);

        let host = host.to_owned();

        let connecting = HttpConnecting {
            state: State::Resolving(self.dns.resolve(host.clone(), port)),
            handle: self.handle.clone(),
        };
        HttpsConnecting(if is_https {
            Box::new(connecting.and_then(move |tcp| {
                TlsConnector::builder()
                    .and_then(|c| c.build())
                    .map(|c| c.connect_async(&host, tcp))
                    .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
            }).and_then(|maybe_tls| {
                maybe_tls.map(|tls| MaybeHttpsStream::Https(tls))
                    .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
            }))
        } else {
            Box::new(connecting.map(|tcp| MaybeHttpsStream::Http(tcp)))
        })
    }

}


/// A Future representing work to connect to a URL.
pub struct HttpConnecting {
    state: State,
    handle: Handle,
}

/// A Future representing work to connect to a URL, and a TLS handshake.
pub struct HttpsConnecting(Box<Future<Item=MaybeHttpsStream, Error=io::Error>>);

enum State {
    Resolving(dns::Query),
    Connecting(ConnectingTcp),
    Error(Option<io::Error>),
}

impl Future for HttpConnecting {
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
                State::Error(ref mut e) => return Err(e.take().expect("polled more than once")),
            }
            self.state = state;
        }
    }
}

impl fmt::Debug for HttpConnecting {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad("HttpConnecting")
    }
}

impl Future for HttpsConnecting {
    type Item = MaybeHttpsStream;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        self.0.poll()
    }
}

impl fmt::Debug for HttpsConnecting {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad("HttpsConnecting")
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

/// A stream that might be protected with TLS.
pub enum MaybeHttpsStream {
    Http(TcpStream),
    Https(HttpsStream),
}

impl fmt::Debug for MaybeHttpsStream {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            MaybeHttpsStream::Http(..) => f.pad("Http(..)"),
            MaybeHttpsStream::Https(..) => f.pad("Https(..)"),
        }
    }
}

impl Read for MaybeHttpsStream {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match *self {
            MaybeHttpsStream::Http(ref mut s) => s.read(buf),
            MaybeHttpsStream::Https(ref mut s) => s.read(buf),
        }
    }
}

impl Write for MaybeHttpsStream {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match *self {
            MaybeHttpsStream::Http(ref mut s) => s.write(buf),
            MaybeHttpsStream::Https(ref mut s) => s.write(buf),
        }
    }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        match *self {
            MaybeHttpsStream::Http(ref mut s) => s.flush(),
            MaybeHttpsStream::Https(ref mut s) => s.flush(),
        }
    }
}

impl Io for MaybeHttpsStream {
    #[inline]
    fn poll_read(&mut self) -> Async<()> {
        match *self {
            MaybeHttpsStream::Http(ref mut s) => s.poll_read(),
            MaybeHttpsStream::Https(ref mut s) => s.poll_read(),
        }
    }

    #[inline]
    fn poll_write(&mut self) -> Async<()> {
        match *self {
            MaybeHttpsStream::Http(ref mut s) => s.poll_write(),
            MaybeHttpsStream::Https(ref mut s) => s.poll_write(),
        }
    }
}

/*
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

#[cfg(test)]
mod tests {
    use std::io;
    use tokio::reactor::Core;
    use url::Url;
    use super::{Connect, HttpConnector};

    #[test]
    fn test_non_http_url() {
        let mut core = Core::new().unwrap();
        let url = Url::parse("file:///home/sean/foo.txt").unwrap();
        let mut connector = HttpConnector::new(1, &core.handle());

        assert_eq!(core.run(connector.connect(url)).unwrap_err().kind(), io::ErrorKind::InvalidInput);
    }

}
