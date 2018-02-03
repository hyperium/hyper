//! HTTP Client

use std::cell::Cell;
use std::fmt;
use std::io;
use std::marker::PhantomData;
use std::rc::Rc;
use std::time::Duration;

use futures::{Async, Future, Poll, Stream};
use futures::future::{self, Either, Executor};
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

mod cancel;
mod connect;
//TODO(easy): move cancel and dispatch into common instead
pub(crate) mod dispatch;
mod dns;
mod pool;
#[cfg(feature = "compat")]
pub mod compat;

/// A Client to make outgoing HTTP requests.
// If the Connector is clone, then the Client can be clone easily.
pub struct Client<C, B = proto::Body> {
    connector: C,
    executor: Exec,
    pool: Pool<HyperClient<B>>,
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

    /// Create a new client with a specific connector.
    #[inline]
    fn configured(config: Config<C, B>, exec: Exec) -> Client<C, B> {
        Client {
            connector: config.connector,
            executor: exec,
            pool: Pool::new(config.keep_alive, config.keep_alive_timeout)
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
    pub fn request(&self, req: Request<B>) -> FutureResponse {
        self.call(req)
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
        match req.version() {
            HttpVersion::Http10 |
            HttpVersion::Http11 => (),
            other => {
                error!("Request has unsupported version \"{}\"", other);
                return FutureResponse(Box::new(future::err(::Error::Version)));
            }
        }

        let url = req.uri().clone();
        let domain = match uri::scheme_and_authority(&url) {
            Some(uri) => uri,
            None => {
                return FutureResponse(Box::new(future::err(::Error::Io(
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "invalid URI for Client Request"
                    )
                ))));
            }
        };
        let (mut head, body) = request::split(req);
        if !head.headers.has::<Host>() {
            let host = Host::new(
                domain.host().expect("authority implies host").to_owned(),
                domain.port(),
            );
            head.headers.set_pos(0, host);
        }

        let checkout = self.pool.checkout(domain.as_ref());
        let connect = {
            let executor = self.executor.clone();
            let pool = self.pool.clone();
            let pool_key = Rc::new(domain.to_string());
            self.connector.connect(url)
                .and_then(move |io| {
                    let (tx, rx) = dispatch::channel();
                    let tx = HyperClient {
                        tx: tx,
                        should_close: Cell::new(true),
                    };
                    let pooled = pool.pooled(pool_key, tx);
                    let conn = proto::Conn::<_, _, proto::ClientTransaction, _>::new(io, pooled.clone());
                    let dispatch = proto::dispatch::Dispatcher::new(proto::dispatch::Client::new(rx), conn);
                    executor.execute(dispatch.map_err(|e| debug!("client connection error: {}", e)))?;
                    Ok(pooled)
                })
        };

        let race = checkout.select(connect)
            .map(|(client, _work)| client)
            .map_err(|(e, _work)| {
                // the Pool Checkout cannot error, so the only error
                // is from the Connector
                // XXX: should wait on the Checkout? Problem is
                // that if the connector is failing, it may be that we
                // never had a pooled stream at all
                e.into()
            });

        let resp = race.and_then(move |client| {
            match client.tx.send((head, body)) {
                Ok(rx) => {
                    client.should_close.set(false);
                    Either::A(rx.then(|res| {
                        match res {
                            Ok(Ok(res)) => Ok(res),
                            Ok(Err(err)) => Err(err),
                            Err(_) => panic!("dispatch dropped without returning error"),
                        }
                    }))
                },
                Err(_) => {
                    error!("pooled connection was not ready, this is a hyper bug");
                    let err = io::Error::new(
                        io::ErrorKind::BrokenPipe,
                        "pool selected dead connection",
                    );
                    Either::B(future::err(::Error::Io(err)))
                }
            }
        });

        FutureResponse(Box::new(resp))
    }

}

impl<C: Clone, B> Clone for Client<C, B> {
    fn clone(&self) -> Client<C, B> {
        Client {
            connector: self.connector.clone(),
            executor: self.executor.clone(),
            pool: self.pool.clone(),
        }
    }
}

impl<C, B> fmt::Debug for Client<C, B> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad("Client")
    }
}

struct HyperClient<B> {
    should_close: Cell<bool>,
    tx: dispatch::Sender<proto::dispatch::ClientMsg<B>, ::Response>,
}

impl<B> Clone for HyperClient<B> {
    fn clone(&self) -> HyperClient<B> {
        HyperClient {
            tx: self.tx.clone(),
            should_close: self.should_close.clone(),
        }
    }
}

impl<B> self::pool::Ready for HyperClient<B> {
    fn poll_ready(&mut self) -> Poll<(), ()> {
        if self.tx.is_closed() {
            Err(())
        } else {
            Ok(Async::Ready(()))
        }
    }
}

impl<B> Drop for HyperClient<B> {
    fn drop(&mut self) {
        if self.should_close.get() {
            self.should_close.set(false);
            self.tx.cancel();
        }
    }
}

/// Configuration for a Client
pub struct Config<C, B> {
    _body_type: PhantomData<B>,
    //connect_timeout: Duration,
    connector: C,
    keep_alive: bool,
    keep_alive_timeout: Option<Duration>,
    //TODO: make use of max_idle config
    max_idle: usize,
    no_proto: bool,
}

/// Phantom type used to signal that `Config` should create a `HttpConnector`.
#[derive(Debug, Clone, Copy)]
pub struct UseDefaultConnector(());

impl Default for Config<UseDefaultConnector, proto::Body> {
    fn default() -> Config<UseDefaultConnector, proto::Body> {
        Config {
            _body_type: PhantomData::<proto::Body>,
            //connect_timeout: Duration::from_secs(10),
            connector: UseDefaultConnector(()),
            keep_alive: true,
            keep_alive_timeout: Some(Duration::from_secs(90)),
            max_idle: 5,
            no_proto: false,
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
            //connect_timeout: self.connect_timeout,
            connector: self.connector,
            keep_alive: self.keep_alive,
            keep_alive_timeout: self.keep_alive_timeout,
            max_idle: self.max_idle,
            no_proto: self.no_proto,
        }
    }

    /// Set the `Connect` type to be used.
    #[inline]
    pub fn connector<CC>(self, val: CC) -> Config<CC, B> {
        Config {
            _body_type: self._body_type,
            //connect_timeout: self.connect_timeout,
            connector: val,
            keep_alive: self.keep_alive,
            keep_alive_timeout: self.keep_alive_timeout,
            max_idle: self.max_idle,
            no_proto: self.no_proto,
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

    /*
    /// Set the timeout for connecting to a URL.
    ///
    /// Default is 10 seconds.
    #[inline]
    pub fn connect_timeout(mut self, val: Duration) -> Config<C, B> {
        self.connect_timeout = val;
        self
    }
    */

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
        self.connector(HttpConnector::new(4, handle)).build(handle)
    }
}

impl<C, B> fmt::Debug for Config<C, B> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Config")
            .field("keep_alive", &self.keep_alive)
            .field("keep_alive_timeout", &self.keep_alive_timeout)
            .field("max_idle", &self.max_idle)
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

