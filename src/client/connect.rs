use std::error::Error as StdError;
use std::fmt;
use std::io;
use std::mem;
//use std::net::SocketAddr;
use std::time::Duration;

use futures::{Future, Poll, Async};
use futures::future::Either;
use tokio_io::{AsyncRead, AsyncWrite};
use tokio::reactor::{Handle, Timeout};
use tokio::net::{TcpStream, TcpStreamNew};
use tokio_service::Service;
use Uri;

use super::dns;

/// A connector creates an Io to a remote address..
///
/// This trait is not implemented directly, and only exists to make
/// the intent clearer. A connector should implement `Service` with
/// `Request=Uri` and `Response: Io` instead.
pub trait Connect: Service<Request=Uri, Error=io::Error> + 'static {
    /// The connected Io Stream.
    type Output: AsyncRead + AsyncWrite + 'static;
    /// A Future that will resolve to the connected Stream.
    type Future: Future<Item=Self::Output, Error=io::Error> + 'static;
    /// Connect to a remote address.
    fn connect(&self, Uri) -> <Self as Connect>::Future;
}

impl<T> Connect for T
where T: Service<Request=Uri, Error=io::Error> + 'static,
      T::Response: AsyncRead + AsyncWrite,
      T::Future: Future<Error=io::Error>,
{
    type Output = T::Response;
    type Future = T::Future;

    fn connect(&self, url: Uri) -> <Self as Connect>::Future {
        self.call(url)
    }
}

/// A connector for the `http` scheme.
#[derive(Clone)]
pub struct HttpConnector {
    dns: dns::Dns,
    enforce_http: bool,
    handle: Handle,
}

impl HttpConnector {

    /// Construct a new HttpConnector.
    ///
    /// Takes number of DNS worker threads.
    #[inline]
    pub fn new(threads: usize, handle: &Handle) -> HttpConnector {
        HttpConnector {
            dns: dns::Dns::new(threads),
            enforce_http: true,
            handle: handle.clone(),
        }
    }

    /// Option to enforce all `Uri`s have the `http` scheme.
    ///
    /// Enabled by default.
    #[inline]
    pub fn enforce_http(&mut self, is_enforced: bool) {
        self.enforce_http = is_enforced;
    }
}

impl fmt::Debug for HttpConnector {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("HttpConnector")
            .finish()
    }
}

impl Service for HttpConnector {
    type Request = Uri;
    type Response = TcpStream;
    type Error = io::Error;
    type Future = HttpConnecting;

    fn call(&self, uri: Uri) -> Self::Future {
        debug!("Http::connect({:?})", uri);

        if self.enforce_http {
            if uri.scheme() != Some("http") {
                return invalid_url(InvalidUrl::NotHttp, &self.handle);
            }
        } else if uri.scheme().is_none() {
            return invalid_url(InvalidUrl::MissingScheme, &self.handle);
        }

        let host = match uri.host() {
            Some(s) => s,
            None => return invalid_url(InvalidUrl::MissingAuthority, &self.handle),
        };
        let port = match uri.port() {
            Some(port) => port,
            None => match uri.scheme() {
                Some("https") => 443,
                _ => 80,
            },
        };

        HttpConnecting {
            state: State::Lazy(self.dns.clone(), host.into(), port),
            handle: self.handle.clone(),
        }
    }
}

#[inline]
fn invalid_url(err: InvalidUrl, handle: &Handle) -> HttpConnecting {
    HttpConnecting {
        state: State::Error(Some(io::Error::new(io::ErrorKind::InvalidInput, err))),
        handle: handle.clone(),
    }
}

#[derive(Debug, Clone, Copy)]
enum InvalidUrl {
    MissingScheme,
    NotHttp,
    MissingAuthority,
}

impl fmt::Display for InvalidUrl {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.description())
    }
}

impl StdError for InvalidUrl {
    fn description(&self) -> &str {
        match *self {
            InvalidUrl::MissingScheme => "invalid URL, missing scheme",
            InvalidUrl::NotHttp => "invalid URL, scheme must be http",
            InvalidUrl::MissingAuthority => "invalid URL, missing domain",
        }
    }
}

/// A Future representing work to connect to a URL.
pub struct HttpConnecting {
    state: State,
    handle: Handle,
}

enum State {
    Lazy(dns::Dns, String, u16),
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
                State::Lazy(ref dns, ref mut host, port) => {
                    let host = mem::replace(host, String::new());
                    state = State::Resolving(dns.resolve(host, port));
                },
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

/// A connector that enforces as connection timeout
#[derive(Debug)]
pub struct TimeoutConnector<T> {
    /// A connector implementing the `Connect` trait
    connector: T,
    /// Handle to be used to set the timeout within tokio's core
    handle: Handle,
    /// Amount of time to wait connecting
    connect_timeout: Duration,
}

impl TimeoutConnector<HttpConnector> {
    /// Construct a new TimeoutConnector
    ///
    /// Takes number of DNS worker threads
    ///
    /// This uses hyper's default `HttpConnector`. If you wish to use something besides the defaults,
    /// create the connector and then use `TimeoutConnector::with_connector`.
    pub fn new(threads: usize, handle: &Handle, timeout: Duration) -> Self {
        let http = HttpConnector::new(threads, handle);
        TimeoutConnector::with_connector(http, handle, timeout)
    }
}

impl<T: Connect> TimeoutConnector<T> {
    /// Construct a new TimeoutConnector with a given connector implementing the `Connect` trait
    pub fn with_connector(connector: T, handle: &Handle, timeout: Duration) -> Self {
        TimeoutConnector {
            connector: connector,
            handle: handle.clone(),
            connect_timeout: timeout,
        }
    }
}

impl<T> Service for TimeoutConnector<T>
    where T: Service<Error=io::Error> + 'static,
          T::Response: AsyncRead + AsyncWrite,
          T::Future: Future<Error=io::Error>,
{
    type Request = T::Request;
    type Response = T::Response;
    type Error = T::Error;
    type Future = Box<Future<Item=Self::Response, Error=Self::Error>>;

    fn call(&self, req: Self::Request) -> Self::Future {
        let connecting = self.connector.call(req);
        let timeout = Timeout::new(self.connect_timeout, &self.handle).unwrap();

        Box::new(connecting.select2(timeout).then(|res| {
            match res {
                Ok(Either::A((connecting, _))) => Ok(connecting),
                Ok(Either::B((_, _))) => {
                    Err(io::Error::new(
                            io::ErrorKind::TimedOut,
                            "Client timed out while connecting"
                        ))
                }
                Err(Either::A((e, _))) => Err(e),
                Err(Either::B((e, _))) => Err(e),
            }
        }))
    }
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::time::Duration;
    use tokio::reactor::Core;
    use super::{Connect, HttpConnector, TimeoutConnector};

    #[test]
    fn test_errors_missing_authority() {
        let mut core = Core::new().unwrap();
        let url = "/foo/bar?baz".parse().unwrap();
        let connector = HttpConnector::new(1, &core.handle());

        assert_eq!(core.run(connector.connect(url)).unwrap_err().kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn test_errors_enforce_http() {
        let mut core = Core::new().unwrap();
        let url = "https://example.domain/foo/bar?baz".parse().unwrap();
        let connector = HttpConnector::new(1, &core.handle());

        assert_eq!(core.run(connector.connect(url)).unwrap_err().kind(), io::ErrorKind::InvalidInput);
    }


    #[test]
    fn test_errors_missing_scheme() {
        let mut core = Core::new().unwrap();
        let url = "example.domain".parse().unwrap();
        let connector = HttpConnector::new(1, &core.handle());

        assert_eq!(core.run(connector.connect(url)).unwrap_err().kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn test_timeout_connector() {
        let mut core = Core::new().unwrap();
        // 10.255.255.1 is a not a routable IP address
        let url = "http://10.255.255.1".parse().unwrap();
        let connector = TimeoutConnector::with_connector(
            HttpConnector::new(1, &core.handle()),
            &core.handle(),
            Duration::from_millis(1)
        );

        assert_eq!(core.run(connector.connect(url)).unwrap_err().kind(), io::ErrorKind::TimedOut);
    }
}
