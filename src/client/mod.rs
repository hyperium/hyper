//! HTTP Client

use std::fmt;
use std::io;
use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use futures::{Async, Future, Poll, Stream};
use futures::future::{self, Executor};
#[cfg(feature = "compat")]
use http;
use tokio::reactor::Handle;
pub use tokio_service::Service;

use header::{Host};
use proto;
use proto::request;
use method::Method;
use self::pool::Pool;
use uri::{self, Uri};
use version::HttpVersion;

pub use proto::response::Response;
pub use proto::request::Request;
pub use self::connect::{HttpConnector, Connect};

use self::background::{bg, Background};

pub mod conn;
mod connect;
//TODO(easy): move cancel and dispatch into common instead
pub(crate) mod dispatch;
mod dns;
mod pool;
#[cfg(feature = "compat")]
pub mod compat;
mod signal;
#[cfg(test)]
mod tests;

/// A Client to make outgoing HTTP requests.
pub struct Client<C, B = proto::Body> {
    connector: Rc<C>,
    executor: Exec,
    h1_writev: bool,
    pool: Pool<PoolClient<B>>,
    retry_canceled_requests: bool,
    set_host: bool,
}

impl Client<HttpConnector, proto::Body> {
    /// Create a new Client with the default config.
    #[inline]
    pub fn new(handle: &Handle) -> Client<HttpConnector, proto::Body> {
        Config::default().build(handle)
    }
}

impl Client<HttpConnector, proto::Body> {
    /// Configure a Client.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # extern crate hyper;
    /// # extern crate tokio_core;
    ///
    /// # fn main() {
    /// # let core = tokio_core::reactor::Core::new().unwrap();
    /// # let handle = core.handle();
    /// let client = hyper::Client::configure()
    ///     .keep_alive(true)
    ///     .build(&handle);
    /// # drop(client);
    /// # }
    /// ```
    #[inline]
    pub fn configure() -> Config<UseDefaultConnector, proto::Body> {
        Config::default()
    }
}

impl<C, B> Client<C, B> {
    // Eventually, a Client won't really care about a tokio Handle, and only
    // the executor used to spawn background tasks. Removing this method is
    // a breaking change, so for now, it's just deprecated.
    #[doc(hidden)]
    #[deprecated]
    pub fn handle(&self) -> &Handle {
        match self.executor {
            Exec::Handle(ref h) => h,
            Exec::Executor(..) => panic!("Client not built with a Handle"),
        }
    }

    #[inline]
    fn configured(config: Config<C, B>, exec: Exec) -> Client<C, B> {
        Client {
            connector: Rc::new(config.connector),
            executor: exec,
            h1_writev: config.h1_writev,
            pool: Pool::new(config.keep_alive, config.keep_alive_timeout),
            retry_canceled_requests: config.retry_canceled_requests,
            set_host: config.set_host,
        }
    }
}

impl<C, B> Client<C, B>
where C: Connect,
      B: Stream<Error=::Error> + 'static,
      B::Item: AsRef<[u8]>,
{
    /// Send a GET Request using this Client.
    #[inline]
    pub fn get(&self, url: Uri) -> FutureResponse {
        self.request(Request::new(Method::Get, url))
    }

    /// Send a constructed Request using this Client.
    #[inline]
    pub fn request(&self, mut req: Request<B>) -> FutureResponse {
        // TODO(0.12): do this at construction time.
        //
        // It cannot be done in the constructor because the Client::configured
        // does not have `B: 'static` bounds, which are required to spawn
        // the interval. In 0.12, add a static bounds to the constructor,
        // and move this.
        self.schedule_pool_timer();

        match req.version() {
            HttpVersion::Http10 |
            HttpVersion::Http11 => (),
            other => {
                error!("Request has unsupported version \"{}\"", other);
                return FutureResponse(Box::new(future::err(::Error::Version)));
            }
        }

        if req.method() == &Method::Connect {
            debug!("Client does not support CONNECT requests");
            return FutureResponse(Box::new(future::err(::Error::Method)));
        }

        let domain = match uri::scheme_and_authority(req.uri()) {
            Some(uri) => uri,
            None => {
                debug!("request uri does not include scheme and authority");
                return FutureResponse(Box::new(future::err(::Error::Io(
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "invalid URI for Client Request"
                    )
                ))));
            }
        };
        if self.set_host && !req.headers().has::<Host>() {
            let host = Host::new(
                domain.host().expect("authority implies host").to_owned(),
                domain.port(),
            );
            req.headers_mut().set_pos(0, host);
        }

        let client = self.clone();
        let is_proxy = req.is_proxy();
        let uri = req.uri().clone();
        let fut = RetryableSendRequest {
            client: client,
            future: self.send_request(req, &domain),
            domain: domain,
            is_proxy: is_proxy,
            uri: uri,
        };
        FutureResponse(Box::new(fut))
    }

    /// Send an `http::Request` using this Client.
    #[inline]
    #[cfg(feature = "compat")]
    pub fn request_compat(&self, req: http::Request<B>) -> compat::CompatFutureResponse {
        self::compat::future(self.call(req.into()))
    }

    /// Convert into a client accepting `http::Request`.
    #[cfg(feature = "compat")]
    pub fn into_compat(self) -> compat::CompatClient<C, B> {
        self::compat::client(self)
    }

    //TODO: replace with `impl Future` when stable
    fn send_request(&self, req: Request<B>, domain: &Uri) -> Box<Future<Item=Response, Error=ClientError<B>>> {
    //fn send_request(&self, req: Request<B>, domain: &Uri) -> Box<Future<Item=Response, Error=::Error>> {
        let url = req.uri().clone();
        let checkout = self.pool.checkout(domain.as_ref());
        let connect = {
            let executor = self.executor.clone();
            let pool = self.pool.clone();
            let pool_key = Arc::new(domain.to_string());
            let h1_writev = self.h1_writev;
            let connector = self.connector.clone();
            future::lazy(move || {
                connector.connect(url)
                    .from_err()
                    .and_then(move |io| {
                        conn::Builder::new()
                            .h1_writev(h1_writev)
                            .handshake_no_upgrades(io)
                    }).and_then(move |(tx, conn)| {
                        executor.execute(conn.map_err(|e| debug!("client connection error: {}", e)))?;
                        Ok(pool.pooled(pool_key, PoolClient {
                            tx: tx,
                        }))
                    })
            })
        };

        let race = checkout.select(connect)
            .map(|(pooled, _work)| pooled)
            .map_err(|(e, _checkout)| {
                // the Pool Checkout cannot error, so the only error
                // is from the Connector
                // XXX: should wait on the Checkout? Problem is
                // that if the connector is failing, it may be that we
                // never had a pooled stream at all
                ClientError::Normal(e)
            });


        let executor = self.executor.clone();
        let resp = race.and_then(move |mut pooled| {
            let conn_reused = pooled.is_reused();
            let fut = pooled.tx.send_request_retryable(req)
                .map_err(move |(err, orig_req)| {
                    if let Some(req) = orig_req {
                        ClientError::Canceled {
                            connection_reused: conn_reused,
                            reason: err,
                            req: req,
                        }
                    } else {
                        ClientError::Normal(err)
                    }
                });

            // when pooled is dropped, it will try to insert back into the
            // pool. To delay that, spawn a future that completes once the
            // sender is ready again.
            //
            // This *should* only be once the related `Connection` has polled
            // for a new request to start.
            //
            // If the executor doesn't have room, oh well. Things will likely
            // be blowing up soon, but this specific task isn't required.
            let _ = executor.execute(future::poll_fn(move || {
                pooled.tx.poll_ready().map_err(|_| ())
            }));

            fut
        });

        Box::new(resp)
    }

    fn schedule_pool_timer(&self) {
        if let Exec::Handle(ref h) = self.executor {
            self.pool.spawn_expired_interval(h);
        }
    }
}

impl<C, B> Service for Client<C, B>
where C: Connect,
      B: Stream<Error=::Error> + 'static,
      B::Item: AsRef<[u8]>,
{
    type Request = Request<B>;
    type Response = Response;
    type Error = ::Error;
    type Future = FutureResponse;

    fn call(&self, req: Self::Request) -> Self::Future {
        self.request(req)
    }
}

impl<C, B> Clone for Client<C, B> {
    fn clone(&self) -> Client<C, B> {
        Client {
            connector: self.connector.clone(),
            executor: self.executor.clone(),
            h1_writev: self.h1_writev,
            pool: self.pool.clone(),
            retry_canceled_requests: self.retry_canceled_requests,
            set_host: self.set_host,
        }
    }
}

impl<C, B> fmt::Debug for Client<C, B> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Client")
            .finish()
    }
}

/// A `Future` that will resolve to an HTTP Response.
#[must_use = "futures do nothing unless polled"]
pub struct FutureResponse(Box<Future<Item=Response, Error=::Error> + 'static>);

impl fmt::Debug for FutureResponse {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad("Future<Response>")
    }
}

impl Future for FutureResponse {
    type Item = Response;
    type Error = ::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        self.0.poll()
    }
}

struct RetryableSendRequest<C, B> {
    client: Client<C, B>,
    domain: Uri,
    future: Box<Future<Item=Response, Error=ClientError<B>>>,
    is_proxy: bool,
    uri: Uri,
}

impl<C, B> Future for RetryableSendRequest<C, B>
where
    C: Connect,
    B: Stream<Error=::Error> + 'static,
    B::Item: AsRef<[u8]>,
{
    type Item = Response;
    type Error = ::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        loop {
            match self.future.poll() {
                Ok(Async::Ready(resp)) => return Ok(Async::Ready(resp)),
                Ok(Async::NotReady) => return Ok(Async::NotReady),
                Err(ClientError::Normal(err)) => return Err(err),
                Err(ClientError::Canceled {
                    connection_reused,
                    req,
                    reason,
                }) => {
                    if !self.client.retry_canceled_requests || !connection_reused {
                        // if client disabled, don't retry
                        // a fresh connection means we definitely can't retry
                        return Err(reason);
                    }

                    trace!("unstarted request canceled, trying again (reason={:?})", reason);
                    let mut req = request::join(req);
                    req.set_proxy(self.is_proxy);
                    req.set_uri(self.uri.clone());
                    self.future = self.client.send_request(req, &self.domain);
                }
            }
        }
    }
}

struct PoolClient<B> {
    tx: conn::SendRequest<B>,
}

impl<B> self::pool::Closed for PoolClient<B>
where
    B: 'static,
{
    fn is_closed(&self) -> bool {
        self.tx.is_closed()
    }
}

pub(crate) enum ClientError<B> {
    Normal(::Error),
    Canceled {
        connection_reused: bool,
        req: (::proto::RequestHead, Option<B>),
        reason: ::Error,
    }
}

/// Configuration for a Client
pub struct Config<C, B> {
    _body_type: PhantomData<B>,
    //connect_timeout: Duration,
    connector: C,
    keep_alive: bool,
    keep_alive_timeout: Option<Duration>,
    h1_writev: bool,
    //TODO: make use of max_idle config
    max_idle: usize,
    retry_canceled_requests: bool,
    set_host: bool,
}

/// Phantom type used to signal that `Config` should create a `HttpConnector`.
#[derive(Debug, Clone, Copy)]
pub struct UseDefaultConnector(());

impl Default for Config<UseDefaultConnector, proto::Body> {
    fn default() -> Config<UseDefaultConnector, proto::Body> {
        Config {
            _body_type: PhantomData::<proto::Body>,
            connector: UseDefaultConnector(()),
            keep_alive: true,
            keep_alive_timeout: Some(Duration::from_secs(90)),
            h1_writev: true,
            max_idle: 5,
            retry_canceled_requests: true,
            set_host: true,
        }
    }
}

impl<C, B> Config<C, B> {
    /// Set the body stream to be used by the `Client`.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use hyper::client::Config;
    /// let cfg = Config::default()
    ///     .body::<hyper::Body>();
    /// # drop(cfg);
    #[inline]
    pub fn body<BB>(self) -> Config<C, BB> {
        Config {
            _body_type: PhantomData::<BB>,
            connector: self.connector,
            keep_alive: self.keep_alive,
            keep_alive_timeout: self.keep_alive_timeout,
            h1_writev: self.h1_writev,
            max_idle: self.max_idle,
            retry_canceled_requests: self.retry_canceled_requests,
            set_host: self.set_host,
        }
    }

    /// Set the `Connect` type to be used.
    #[inline]
    pub fn connector<CC>(self, val: CC) -> Config<CC, B> {
        Config {
            _body_type: self._body_type,
            connector: val,
            keep_alive: self.keep_alive,
            keep_alive_timeout: self.keep_alive_timeout,
            h1_writev: self.h1_writev,
            max_idle: self.max_idle,
            retry_canceled_requests: self.retry_canceled_requests,
            set_host: self.set_host,
        }
    }

    /// Enable or disable keep-alive mechanics.
    ///
    /// Default is enabled.
    #[inline]
    pub fn keep_alive(mut self, val: bool) -> Config<C, B> {
        self.keep_alive = val;
        self
    }

    /// Set an optional timeout for idle sockets being kept-alive.
    ///
    /// Pass `None` to disable timeout.
    ///
    /// Default is 90 seconds.
    #[inline]
    pub fn keep_alive_timeout(mut self, val: Option<Duration>) -> Config<C, B> {
        self.keep_alive_timeout = val;
        self
    }

    /// Set whether HTTP/1 connections should try to use vectored writes,
    /// or always flatten into a single buffer.
    ///
    /// Note that setting this to false may mean more copies of body data,
    /// but may also improve performance when an IO transport doesn't
    /// support vectored writes well, such as most TLS implementations.
    ///
    /// Default is `true`.
    #[inline]
    pub fn http1_writev(mut self, val: bool) -> Config<C, B> {
        self.h1_writev = val;
        self
    }

    /// Set whether to retry requests that get disrupted before ever starting
    /// to write.
    ///
    /// This means a request that is queued, and gets given an idle, reused
    /// connection, and then encounters an error immediately as the idle
    /// connection was found to be unusable.
    ///
    /// When this is set to `false`, the related `FutureResponse` would instead
    /// resolve to an `Error::Cancel`.
    ///
    /// Default is `true`.
    #[inline]
    pub fn retry_canceled_requests(mut self, val: bool) -> Config<C, B> {
        self.retry_canceled_requests = val;
        self
    }

    /// Set whether to automatically add the `Host` header to requests.
    ///
    /// If true, and a request does not include a `Host` header, one will be
    /// added automatically, derived from the authority of the `Uri`.
    ///
    /// Default is `true`.
    #[inline]
    pub fn set_host(mut self, val: bool) -> Config<C, B> {
        self.set_host = val;
        self
    }

    #[doc(hidden)]
    #[deprecated(since="0.11.11", note="no_proto is always enabled")]
    pub fn no_proto(self) -> Config<C, B> {
        self
    }
}

impl<C, B> Config<C, B>
where C: Connect,
      B: Stream<Error=::Error>,
      B::Item: AsRef<[u8]>,
{
    /// Construct the Client with this configuration.
    #[inline]
    pub fn build(self, handle: &Handle) -> Client<C, B> {
        Client::configured(self, Exec::Handle(handle.clone()))
    }

    /// Construct a Client with this configuration and an executor.
    ///
    /// The executor will be used to spawn "background" connection tasks
    /// to drive requests and responses.
    pub fn executor<E>(self, executor: E) -> Client<C, B>
    where
        E: Executor<Background> + 'static,
    {
        Client::configured(self, Exec::Executor(Rc::new(executor)))
    }
}

impl<B> Config<UseDefaultConnector, B>
where B: Stream<Error=::Error>,
      B::Item: AsRef<[u8]>,
{
    /// Construct the Client with this configuration.
    #[inline]
    pub fn build(self, handle: &Handle) -> Client<HttpConnector, B> {
        let mut connector = HttpConnector::new(4, handle);
        if self.keep_alive {
            connector.set_keepalive(self.keep_alive_timeout);
        }
        self.connector(connector).build(handle)
    }
}

impl<C, B> fmt::Debug for Config<C, B> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Config")
            .field("keep_alive", &self.keep_alive)
            .field("keep_alive_timeout", &self.keep_alive_timeout)
            .field("http1_writev", &self.h1_writev)
            .field("max_idle", &self.max_idle)
            .field("set_host", &self.set_host)
            .finish()
    }
}

impl<C: Clone, B> Clone for Config<C, B> {
    fn clone(&self) -> Config<C, B> {
        Config {
            connector: self.connector.clone(),
            .. *self
        }
    }
}


// ===== impl Exec =====

#[derive(Clone)]
enum Exec {
    Handle(Handle),
    Executor(Rc<Executor<Background>>),
}


impl Exec {
    fn execute<F>(&self, fut: F) -> io::Result<()>
    where
        F: Future<Item=(), Error=()> + 'static,
    {
        match *self {
            Exec::Handle(ref h) => h.spawn(fut),
            Exec::Executor(ref e) => {
                e.execute(bg(Box::new(fut)))
                    .map_err(|err| {
                        debug!("executor error: {:?}", err.kind());
                        io::Error::new(
                            io::ErrorKind::Other,
                            "executor error",
                        )
                    })?
            },
        }
        Ok(())
    }
}

// ===== impl Background =====

// The types inside this module are not exported out of the crate,
// so they are in essence un-nameable.
mod background {
    use futures::{Future, Poll};

    // This is basically `impl Future`, since the type is un-nameable,
    // and only implementeds `Future`.
    #[allow(missing_debug_implementations)]
    pub struct Background {
        inner: Box<Future<Item=(), Error=()>>,
    }

    pub fn bg(fut: Box<Future<Item=(), Error=()>>) -> Background {
        Background {
            inner: fut,
        }
    }

    impl Future for Background {
        type Item = ();
        type Error = ();

        fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
            self.inner.poll()
        }
    }
}

