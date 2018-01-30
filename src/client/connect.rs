//! Contains the `Connect2` trait, and supporting types.
use std::error::Error as StdError;
use std::fmt;
use std::io;
use std::mem;
use std::sync::Arc;

use futures::{Future, Poll, Async};
use futures::future::{Executor, ExecuteError};
use futures::sync::oneshot;
use futures_cpupool::{Builder as CpuPoolBuilder};
use tokio_io::{AsyncRead, AsyncWrite};
use tokio::reactor::Handle;
use tokio::net::{TcpStream, TcpStreamNew};
use tokio_service::Service;
use Uri;

use super::dns;
use self::http_connector::HttpConnectorBlockingTask;

/// Connect to a destination, returning an IO transport.
pub trait Connect2 {
    /// The connected IO Stream.
    type Transport: AsyncRead + AsyncWrite;
    /// An error occured when trying to connect.
    type Error;
    /// A Future that will resolve to the connected Transport.
    type Future: Future<Item=(Self::Transport, Connected), Error=Self::Error>;
    /// Connect to a destination.
    fn connect(&self, dst: Destination) -> Self::Future;
}

/// A set of properties to describe where and how to try to connect.
#[derive(Debug)]
pub struct Destination {
    pub(super) alpn: Alpn,
    pub(super) uri: Uri,
}

/// Extra information about the connected transport.
#[derive(Debug)]
pub struct Connected {
    alpn: Alpn,
    is_proxy: bool,
}

#[derive(Debug)]
pub(super) enum Alpn {
    Http1,
    H2,
}

impl Destination {
    /// Get a reference to the requested `Uri`.
    pub fn uri(&self) -> &Uri {
        &self.uri
    }

    /// Returns whether this connection must negotiate HTTP/2 via ALPN.
    pub fn h2(&self) -> bool {
        match self.alpn {
            Alpn::Http1 => false,
            Alpn::H2 => true,
        }
    }
}

impl Connected {
    /// Create new `Connected` type with empty metadata.
    pub fn new() -> Connected {
        Connected {
            alpn: Alpn::Http1,
            is_proxy: false,
        }
    }

    /// Set that the connected transport is to an HTTP proxy.
    ///
    /// This setting will affect if HTTP/1 requests written on the transport
    /// will have the request-target in absolute-form or origin-form (such as
    /// `GET http://hyper.rs/guide HTTP/1.1` or `GET /guide HTTP/1.1`).
    pub fn proxy(mut self) -> Connected {
        self.is_proxy = true;
        self
    }

    /// Set that the connected transport negotiated HTTP/2 as it's
    /// next protocol.
    pub fn h2(mut self) -> Connected {
        self.alpn = Alpn::H2;
        self
    }
}

/// A connector for the `http` scheme.
#[derive(Clone)]
pub struct HttpConnector {
    executor: HttpConnectExecutor,
    enforce_http: bool,
    handle: Handle,
}

impl HttpConnector {
    /// Construct a new HttpConnector.
    ///
    /// Takes number of DNS worker threads.
    #[inline]
    pub fn new(threads: usize, handle: &Handle) -> HttpConnector {
        let pool = CpuPoolBuilder::new()
            .name_prefix("hyper-dns")
            .pool_size(threads)
            .create();
        HttpConnector::new_with_executor(pool, handle)
    }

    /// Construct a new HttpConnector.
    ///
    /// Takes an executor to run blocking tasks on.
    #[inline]
    pub fn new_with_executor<E: 'static>(executor: E, handle: &Handle) -> HttpConnector
        where E: Executor<HttpConnectorBlockingTask>
    {
        HttpConnector {
            executor: HttpConnectExecutor(Arc::new(executor)),
            enforce_http: true,
            handle: handle.clone()
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

// deprecated, will be gone in 0.12
#[doc(hidden)]
impl Service for HttpConnector {
    type Request = Uri;
    type Response = TcpStream;
    type Error = io::Error;
    type Future = HttpConnecting;

    fn call(&self, uri: Uri) -> Self::Future {
        trace!("Http::connect({:?})", uri);

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
            state: State::Lazy(self.executor.clone(), host.into(), port),
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
#[must_use = "futures do nothing unless polled"]
pub struct HttpConnecting {
    state: State,
    handle: Handle,
}

enum State {
    Lazy(HttpConnectExecutor, String, u16),
    Resolving(oneshot::SpawnHandle<dns::IpAddrs, io::Error>),
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
                State::Lazy(ref executor, ref mut host, port) => {
                    // If the host is already an IP addr (v4 or v6),
                    // skip resolving the dns and start connecting right away.
                    if let Some(addrs) = dns::IpAddrs::try_parse(host, port) {
                        state = State::Connecting(ConnectingTcp {
                            addrs: addrs,
                            current: None
                        })
                    } else {
                        let host = mem::replace(host, String::new());
                        let work = dns::Work::new(host, port);
                        state = State::Resolving(oneshot::spawn(work, executor));
                    }
                },
                State::Resolving(ref mut future) => {
                    match try!(future.poll()) {
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
                            debug!("connecting to {}", addr);
                            *current = TcpStream::connect(&addr, handle);
                            continue;
                        }
                    }
                }
            } else if let Some(addr) = self.addrs.next() {
                debug!("connecting to {}", addr);
                self.current = Some(TcpStream::connect(&addr, handle));
                continue;
            }

            return Err(err.take().expect("missing connect error"));
        }
    }
}

// Make this Future unnameable outside of this crate.
mod http_connector {
    use super::*;
    // Blocking task to be executed on a thread pool.
    pub struct HttpConnectorBlockingTask {
        pub(super) work: oneshot::Execute<dns::Work>
    }

    impl fmt::Debug for HttpConnectorBlockingTask {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.pad("HttpConnectorBlockingTask")
        }
    }

    impl Future for HttpConnectorBlockingTask {
        type Item = ();
        type Error = ();

        fn poll(&mut self) -> Poll<(), ()> {
            self.work.poll()
        }
    }
}

#[derive(Clone)]
struct HttpConnectExecutor(Arc<Executor<HttpConnectorBlockingTask>>);

impl Executor<oneshot::Execute<dns::Work>> for HttpConnectExecutor {
    fn execute(&self, future: oneshot::Execute<dns::Work>) -> Result<(), ExecuteError<oneshot::Execute<dns::Work>>> {
        self.0.execute(HttpConnectorBlockingTask { work: future })
            .map_err(|err| ExecuteError::new(err.kind(), err.into_future().work))
    }
}

#[doc(hidden)]
#[deprecated(since="0.11.16", note="Use the Connect2 trait, which will become Connect in 0.12")]
pub trait Connect: Service<Request=Uri, Error=io::Error> + 'static {
    /// The connected Io Stream.
    type Output: AsyncRead + AsyncWrite + 'static;
    /// A Future that will resolve to the connected Stream.
    type Future: Future<Item=Self::Output, Error=io::Error> + 'static;
    /// Connect to a remote address.
    fn connect(&self, Uri) -> <Self as Connect>::Future;
}

#[doc(hidden)]
#[allow(deprecated)]
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

#[doc(hidden)]
#[allow(deprecated)]
impl<T> Connect2 for T
where
    T: Connect,
{
    type Transport = <T as Connect>::Output;
    type Error = io::Error;
    type Future = ConnectToConnect2Future<<T as Connect>::Future>;

    fn connect(&self, dst: Destination) -> <Self as Connect2>::Future {
        ConnectToConnect2Future {
            inner: <Self as Connect>::connect(self, dst.uri),
        }
    }
}

#[doc(hidden)]
#[deprecated(since="0.11.16")]
#[allow(missing_debug_implementations)]
pub struct ConnectToConnect2Future<F> {
    inner: F,
}

#[allow(deprecated)]
impl<F> Future for ConnectToConnect2Future<F>
where
    F: Future,
{
    type Item = (F::Item, Connected);
    type Error = F::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        self.inner.poll()
            .map(|async| async.map(|t| (t, Connected::new())))
    }
}

// even though deprecated, we need to make sure the HttpConnector still
// implements Connect (and Service apparently...)

#[allow(deprecated)]
fn _assert_http_connector() {
    fn assert_connect<T>()
    where
        T: Connect2<
            Transport=TcpStream,
            Error=io::Error,
            Future=ConnectToConnect2Future<HttpConnecting>
        >,
        T: Connect<Output=TcpStream, Future=HttpConnecting>,
        T: Service<
            Request=Uri,
            Response=TcpStream,
            Future=HttpConnecting,
            Error=io::Error
        >,
    {}

    assert_connect::<HttpConnector>();
}

#[cfg(test)]
mod tests {
    #![allow(deprecated)]
    use std::io;
    use tokio::reactor::Core;
    use super::{Connect, HttpConnector};

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
}
