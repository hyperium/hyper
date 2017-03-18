use std::fmt;
use std::io;
//use std::net::SocketAddr;

use futures::{Future, Poll, Async};
use tokio_io::{AsyncRead, AsyncWrite};
use tokio::reactor::Handle;
use tokio::net::{TcpStream, TcpStreamNew};
use tokio_service::Service;
use Url;

use super::dns;

/// A connector creates an Io to a remote address..
///
/// This trait is not implemented directly, and only exists to make
/// the intent clearer. A connector should implement `Service` with
/// `Request=Url` and `Response: Io` instead.
pub trait Connect: Service<Request=Url, Error=io::Error> + 'static {
    /// The connected Io Stream.
    type Output: AsyncRead + AsyncWrite + 'static;
    /// A Future that will resolve to the connected Stream.
    type Future: Future<Item=Self::Output, Error=io::Error> + 'static;
    /// Connect to a remote address.
    fn connect(&self, Url) -> <Self as Connect>::Future;
}

impl<T> Connect for T
where T: Service<Request=Url, Error=io::Error> + 'static,
      T::Response: AsyncRead + AsyncWrite,
      T::Future: Future<Error=io::Error>,
{
    type Output = T::Response;
    type Future = T::Future;

    fn connect(&self, url: Url) -> <Self as Connect>::Future {
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

    fn call(&self, url: Url) -> Self::Future {
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

/// A Future representing work to connect to a URL.
pub struct HttpConnecting {
    state: State,
    handle: Handle,
}

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
    use Url;
    use super::{Connect, HttpConnector};

    #[test]
    fn test_non_http_url() {
        let mut core = Core::new().unwrap();
        let url = Url::parse("file:///home/sean/foo.txt").unwrap();
        let connector = HttpConnector::new(1, &core.handle());

        assert_eq!(core.run(connector.connect(url)).unwrap_err().kind(), io::ErrorKind::InvalidInput);
    }

}
