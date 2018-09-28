//! HTTP Client
//!
//! There are two levels of APIs provided for construct HTTP clients:
//!
//! - The higher-level [`Client`](Client) type.
//! - The lower-level [conn](conn) module.
//!
//! # Client
//!
//! The [`Client`](Client) is the main way to send HTTP requests to a server.
//! The default `Client` provides these things on top of the lower-level API:
//!
//! - A default **connector**, able to resolve hostnames and connect to
//!   destinations over plain-text TCP.
//! - A **pool** of existing connections, allowing better performance when
//!   making multiple requests to the same hostname.
//! - Automatic setting of the `Host` header, based on the request `Uri`.
//! - Automatic request **retries** when a pooled connection is closed by the
//!   server before any bytes have been written.
//!
//! Many of these features can configured, by making use of
//! [`Client::builder`](Client::builder).
//!
//! ## Example
//!
//! For a small example program simply fetching a URL, take a look at the
//! [full client example](https://github.com/hyperium/hyper/blob/master/examples/client.rs).
//!
//! ```
//! extern crate hyper;
//!
//! use hyper::Client;
//! # #[cfg(feature = "runtime")]
//! use hyper::rt::{self, Future, Stream};
//!
//! # #[cfg(feature = "runtime")]
//! # fn fetch_httpbin() {
//! let client = Client::new();
//!
//! let fut = client
//!
//!     // Make a GET /ip to 'http://httpbin.org'
//!     .get("http://httpbin.org/ip".parse().unwrap())
//!
//!     // And then, if the request gets a response...
//!     .and_then(|res| {
//!         println!("status: {}", res.status());
//!
//!         // Concatenate the body stream into a single buffer...
//!         // This returns a new future, since we must stream body.
//!         res.into_body().concat2()
//!     })
//!
//!     // And then, if reading the full body succeeds...
//!     .and_then(|body| {
//!         // The body is just bytes, but let's print a string...
//!         let s = ::std::str::from_utf8(&body)
//!             .expect("httpbin sends utf-8 JSON");
//!
//!         println!("body: {}", s);
//!
//!         // and_then requires we return a new Future, and it turns
//!         // out that Result is a Future that is ready immediately.
//!         Ok(())
//!     })
//!
//!     // Map any errors that might have happened...
//!     .map_err(|err| {
//!         println!("error: {}", err);
//!     });
//!
//! // A runtime is needed to execute our asynchronous code. In order to
//! // spawn the future into the runtime, it should already have been
//! // started and running before calling this code.
//! rt::spawn(fut);
//! # }
//! # fn main () {}
//! ```

use std::fmt;
use std::mem;
use std::sync::Arc;
use std::time::Duration;

use futures::{Async, Future, Poll};
use futures::future::{self, Either, Executor};
use futures::sync::oneshot;
use http::{Method, Request, Response, Uri, Version};
use http::header::{Entry, HeaderValue, HOST};
use http::uri::Scheme;

use body::{Body, Payload};
use common::{Exec, lazy as hyper_lazy, Lazy};
use self::connect::{Connect, Destination};
use self::pool::{Key as PoolKey, Pool, Poolable, Pooled, Reservation};

#[cfg(feature = "runtime")] pub use self::connect::HttpConnector;

pub mod conn;
pub mod connect;
pub(crate) mod dispatch;
mod pool;
#[cfg(test)]
mod tests;

/// A Client to make outgoing HTTP requests.
pub struct Client<C, B = Body> {
    connector: Arc<C>,
    executor: Exec,
    h1_writev: bool,
    h1_title_case_headers: bool,
    pool: Pool<PoolClient<B>>,
    retry_canceled_requests: bool,
    set_host: bool,
    ver: Ver,
}

#[cfg(feature = "runtime")]
impl Client<HttpConnector, Body> {
    /// Create a new Client with the default config.
    ///
    /// # Note
    ///
    /// The default connector does **not** handle TLS. Speaking to `https`
    /// destinations will require configuring a connector that implements TLS.
    #[inline]
    pub fn new() -> Client<HttpConnector, Body> {
        Builder::default().build_http()
    }
}

#[cfg(feature = "runtime")]
impl Default for Client<HttpConnector, Body> {
    fn default() -> Client<HttpConnector, Body> {
        Client::new()
    }
}

impl Client<(), Body> {
    /// Configure a Client.
    ///
    /// # Example
    ///
    /// ```
    /// # extern crate hyper;
    /// # #[cfg(feature  = "runtime")]
    /// fn run () {
    /// use hyper::Client;
    ///
    /// let client = Client::builder()
    ///     .keep_alive(true)
    ///     .build_http();
    /// # let infer: Client<_, hyper::Body> = client;
    /// # drop(infer);
    /// # }
    /// # fn main() {}
    /// ```
    #[inline]
    pub fn builder() -> Builder {
        Builder::default()
    }
}

impl<C, B> Client<C, B>
where C: Connect + Sync + 'static,
      C::Transport: 'static,
      C::Future: 'static,
      B: Payload + Send + 'static,
      B::Data: Send,
{

    /// Send a `GET` request to the supplied `Uri`.
    ///
    /// # Note
    ///
    /// This requires that the `Payload` type have a `Default` implementation.
    /// It *should* return an "empty" version of itself, such that
    /// `Payload::is_end_stream` is `true`.
    pub fn get(&self, uri: Uri) -> ResponseFuture
    where
        B: Default,
    {
        let body = B::default();
        if !body.is_end_stream() {
            warn!("default Payload used for get() does not return true for is_end_stream");
        }

        let mut req = Request::new(body);
        *req.uri_mut() = uri;
        self.request(req)
    }

    /// Send a constructed Request using this Client.
    pub fn request(&self, mut req: Request<B>) -> ResponseFuture {
        let is_http_11 = self.ver == Ver::Http1 && match req.version() {
            Version::HTTP_11 => true,
            Version::HTTP_10 => false,
            other => {
                error!("Request has unsupported version \"{:?}\"", other);
                return ResponseFuture::new(Box::new(future::err(::Error::new_user_unsupported_version())));
            }
        };

        let is_http_connect = req.method() == &Method::CONNECT;

        if !is_http_11 && is_http_connect {
            debug!("client does not support CONNECT requests for {:?}", req.version());
            return ResponseFuture::new(Box::new(future::err(::Error::new_user_unsupported_request_method())));
        }


        let uri = req.uri().clone();
        let domain = match (uri.scheme_part(), uri.authority_part()) {
            (Some(scheme), Some(auth)) => {
                format!("{}://{}", scheme, auth)
            }
            (None, Some(auth)) if is_http_connect => {
                let scheme = match auth.port() {
                    Some(443) => {
                        set_scheme(req.uri_mut(), Scheme::HTTPS);
                        "https"
                    },
                    _ => {
                        set_scheme(req.uri_mut(), Scheme::HTTP);
                        "http"
                    },
                };
                format!("{}://{}", scheme, auth)
            },
            _ => {
                debug!("Client requires absolute-form URIs, received: {:?}", uri);
                return ResponseFuture::new(Box::new(future::err(::Error::new_user_absolute_uri_required())))
            }
        };

        if self.set_host && self.ver == Ver::Http1 {
            if let Entry::Vacant(entry) = req.headers_mut().entry(HOST).expect("HOST is always valid header name") {
                let hostname = uri.host().expect("authority implies host");
                let host = if let Some(port) = uri.port() {
                    let s = format!("{}:{}", hostname, port);
                    HeaderValue::from_str(&s)
                } else {
                    HeaderValue::from_str(hostname)
                }.expect("uri host is valid header value");
                entry.insert(host);
            }
        }


        let pool_key = (Arc::new(domain.to_string()), self.ver);
        ResponseFuture::new(Box::new(self.retryably_send_request(req, pool_key)))
    }

    fn retryably_send_request(&self, req: Request<B>, pool_key: PoolKey) -> impl Future<Item=Response<Body>, Error=::Error> {
        let client = self.clone();
        let uri = req.uri().clone();

        let mut send_fut = client.send_request(req, pool_key.clone());
        future::poll_fn(move || loop {
            match send_fut.poll() {
                Ok(Async::Ready(resp)) => return Ok(Async::Ready(resp)),
                Ok(Async::NotReady) => return Ok(Async::NotReady),
                Err(ClientError::Normal(err)) => return Err(err),
                Err(ClientError::Canceled {
                    connection_reused,
                    mut req,
                    reason,
                }) => {
                    if !client.retry_canceled_requests || !connection_reused {
                        // if client disabled, don't retry
                        // a fresh connection means we definitely can't retry
                        return Err(reason);
                    }

                    trace!("unstarted request canceled, trying again (reason={:?})", reason);
                    *req.uri_mut() = uri.clone();
                    send_fut = client.send_request(req, pool_key.clone());
                }
            }
        })
    }

    fn send_request(&self, mut req: Request<B>, pool_key: PoolKey) -> impl Future<Item=Response<Body>, Error=ClientError<B>> {
        let race = self.pool_checkout_or_connect(req.uri().clone(), pool_key);

        let ver = self.ver;
        let executor = self.executor.clone();
        race.and_then(move |mut pooled| {
            if ver == Ver::Http1 {
                // CONNECT always sends origin-form, so check it first...
                if req.method() == &Method::CONNECT {
                    authority_form(req.uri_mut());
                } else if pooled.is_proxied {
                    absolute_form(req.uri_mut());
                } else {
                    origin_form(req.uri_mut());
                };
            } else {
                debug_assert!(
                    req.method() != &Method::CONNECT,
                    "Client should have returned Error for HTTP2 CONNECT"
                );
            }

            let fut = pooled.send_request_retryable(req)
                .map_err(ClientError::map_with_reused(pooled.is_reused()));

            // As of futures@0.1.21, there is a race condition in the mpsc
            // channel, such that sending when the receiver is closing can
            // result in the message being stuck inside the queue. It won't
            // ever notify until the Sender side is dropped.
            //
            // To counteract this, we must check if our senders 'want' channel
            // has been closed after having tried to send. If so, error out...
            if pooled.is_closed() {
                return Either::A(fut);
            }

            Either::B(fut
                .and_then(move |mut res| {
                    // If pooled is HTTP/2, we can toss this reference immediately.
                    //
                    // when pooled is dropped, it will try to insert back into the
                    // pool. To delay that, spawn a future that completes once the
                    // sender is ready again.
                    //
                    // This *should* only be once the related `Connection` has polled
                    // for a new request to start.
                    //
                    // It won't be ready if there is a body to stream.
                    if ver == Ver::Http2 || !pooled.is_pool_enabled() || pooled.is_ready() {
                        drop(pooled);
                    } else if !res.body().is_end_stream() {
                        let (delayed_tx, delayed_rx) = oneshot::channel();
                        res.body_mut().delayed_eof(delayed_rx);
                        let on_idle = future::poll_fn(move || {
                            pooled.poll_ready()
                        })
                            .then(move |_| {
                                // At this point, `pooled` is dropped, and had a chance
                                // to insert into the pool (if conn was idle)
                                drop(delayed_tx);
                                Ok(())
                            });

                        if let Err(err) = executor.execute(on_idle) {
                            // This task isn't critical, so just log and ignore.
                            warn!("error spawning task to insert idle connection: {}", err);
                        }
                    } else {
                        // There's no body to delay, but the connection isn't
                        // ready yet. Only re-insert when it's ready
                        let on_idle = future::poll_fn(move || {
                            pooled.poll_ready()
                        })
                            .then(|_| Ok(()));

                        if let Err(err) = executor.execute(on_idle) {
                            // This task isn't critical, so just log and ignore.
                            warn!("error spawning task to insert idle connection: {}", err);
                        }
                    }
                    Ok(res)
                }))
        })
    }

    fn pool_checkout_or_connect(&self, uri: Uri, pool_key: PoolKey)
        -> impl Future<Item=Pooled<PoolClient<B>>, Error=ClientError<B>>
    {
        let checkout = self.pool.checkout(pool_key.clone());
        let connect = self.connect_to(uri, pool_key);

        let executor = self.executor.clone();
        checkout
            // The order of the `select` is depended on below...
            .select2(connect)
            .map(move |either| match either {
                // Checkout won, connect future may have been started or not.
                //
                // If it has, let it finish and insert back into the pool,
                // so as to not waste the socket...
                Either::A((checked_out, connecting)) => {
                    // This depends on the `select` above having the correct
                    // order, such that if the checkout future were ready
                    // immediately, the connect future will never have been
                    // started.
                    //
                    // If it *wasn't* ready yet, then the connect future will
                    // have been started...
                    if connecting.started() {
                        let bg = connecting
                            .map(|_pooled| {
                                // dropping here should just place it in
                                // the Pool for us...
                            })
                            .map_err(|err| {
                                trace!("background connect error: {}", err);
                            });
                        // An execute error here isn't important, we're just trying
                        // to prevent a waste of a socket...
                        let _ = executor.execute(bg);
                    }
                    checked_out
                },
                // Connect won, checkout can just be dropped.
                Either::B((connected, _checkout)) => {
                    connected
                },
            })
            .or_else(|either| match either {
                // Either checkout or connect could get canceled:
                //
                // 1. Connect is canceled if this is HTTP/2 and there is
                //    an outstanding HTTP/2 connecting task.
                // 2. Checkout is canceled if the pool cannot deliver an
                //    idle connection reliably.
                //
                // In both cases, we should just wait for the other future.
                Either::A((err, connecting)) => {
                    if err.is_canceled() {
                        Either::A(Either::A(connecting.map_err(ClientError::Normal)))
                    } else {
                        Either::B(future::err(ClientError::Normal(err)))
                    }
                },
                Either::B((err, checkout)) => {
                    if err.is_canceled() {
                        Either::A(Either::B(checkout.map_err(ClientError::Normal)))
                    } else {
                        Either::B(future::err(ClientError::Normal(err)))
                    }
                }
            })
    }

    fn connect_to(&self, uri: Uri, pool_key: PoolKey)
        -> impl Lazy<Item=Pooled<PoolClient<B>>, Error=::Error>
    {
        let executor = self.executor.clone();
        let pool = self.pool.clone();
        let h1_writev = self.h1_writev;
        let h1_title_case_headers = self.h1_title_case_headers;
        let connector = self.connector.clone();
        let ver = pool_key.1;
        let dst = Destination {
            uri,
        };
        hyper_lazy(move || {
            // Try to take a "connecting lock".
            //
            // If the pool_key is for HTTP/2, and there is already a
            // connection being estabalished, then this can't take a
            // second lock. The "connect_to" future is Canceled.
            let connecting = match pool.connecting(&pool_key) {
                Some(lock) => lock,
                None => {
                    let canceled = ::Error::new_canceled(Some("HTTP/2 connection in progress"));
                    return Either::B(future::err(canceled));
                }
            };
            Either::A(connector.connect(dst)
                .map_err(::Error::new_connect)
                .and_then(move |(io, connected)| {
                    conn::Builder::new()
                        .exec(executor.clone())
                        .h1_writev(h1_writev)
                        .h1_title_case_headers(h1_title_case_headers)
                        .http2_only(pool_key.1 == Ver::Http2)
                        .handshake(io)
                        .and_then(move |(tx, conn)| {
                            let bg = executor.execute(conn.map_err(|e| {
                                debug!("client connection error: {}", e)
                            }));

                            // This task is critical, so an execute error
                            // should be returned.
                            if let Err(err) = bg {
                                warn!("error spawning critical client task: {}", err);
                                return Either::A(future::err(err));
                            }

                            // Wait for 'conn' to ready up before we
                            // declare this tx as usable
                            Either::B(tx.when_ready())
                        })
                        .map(move |tx| {
                            pool.pooled(connecting, PoolClient {
                                is_proxied: connected.is_proxied,
                                tx: match ver {
                                    Ver::Http1 => PoolTx::Http1(tx),
                                    Ver::Http2 => PoolTx::Http2(tx.into_http2()),
                                },
                            })
                        })
                }))
        })
    }
}

impl<C, B> Clone for Client<C, B> {
    fn clone(&self) -> Client<C, B> {
        Client {
            connector: self.connector.clone(),
            executor: self.executor.clone(),
            h1_writev: self.h1_writev,
            h1_title_case_headers: self.h1_title_case_headers,
            pool: self.pool.clone(),
            retry_canceled_requests: self.retry_canceled_requests,
            set_host: self.set_host,
            ver: self.ver,
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
pub struct ResponseFuture {
    inner: Box<Future<Item=Response<Body>, Error=::Error> + Send>,
}

impl ResponseFuture {
    fn new(fut: Box<Future<Item=Response<Body>, Error=::Error> + Send>) -> Self {
        Self {
            inner: fut,
        }
    }
}

impl fmt::Debug for ResponseFuture {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad("Future<Response>")
    }
}

impl Future for ResponseFuture {
    type Item = Response<Body>;
    type Error = ::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        self.inner.poll()
    }
}

// FIXME: allow() required due to `impl Trait` leaking types to this lint
#[allow(missing_debug_implementations)]
struct PoolClient<B> {
    is_proxied: bool,
    tx: PoolTx<B>,
}

enum PoolTx<B> {
    Http1(conn::SendRequest<B>),
    Http2(conn::Http2SendRequest<B>),
}

impl<B> PoolClient<B> {
    fn poll_ready(&mut self) -> Poll<(), ::Error> {
        match self.tx {
            PoolTx::Http1(ref mut tx) => tx.poll_ready(),
            PoolTx::Http2(_) => Ok(Async::Ready(())),
        }
    }

    fn is_ready(&self) -> bool {
        match self.tx {
            PoolTx::Http1(ref tx) => tx.is_ready(),
            PoolTx::Http2(ref tx) => tx.is_ready(),
        }
    }

    fn is_closed(&self) -> bool {
        match self.tx {
            PoolTx::Http1(ref tx) => tx.is_closed(),
            PoolTx::Http2(ref tx) => tx.is_closed(),
        }
    }
}

impl<B: Payload + 'static> PoolClient<B> {
    fn send_request_retryable(&mut self, req: Request<B>) -> impl Future<Item = Response<Body>, Error = (::Error, Option<Request<B>>)>
    where
        B: Send,
    {
        match self.tx {
            PoolTx::Http1(ref mut tx) => Either::A(tx.send_request_retryable(req)),
            PoolTx::Http2(ref mut tx) => Either::B(tx.send_request_retryable(req)),
        }
    }
}

impl<B> Poolable for PoolClient<B>
where
    B: Send + 'static,
{
    fn is_open(&self) -> bool {
        match self.tx {
            PoolTx::Http1(ref tx) => tx.is_ready(),
            PoolTx::Http2(ref tx) => tx.is_ready(),
        }
    }

    fn reserve(self) -> Reservation<Self> {
        match self.tx {
            PoolTx::Http1(tx) => {
                Reservation::Unique(PoolClient {
                    is_proxied: self.is_proxied,
                    tx: PoolTx::Http1(tx),
                })
            },
            PoolTx::Http2(tx) => {
                let b = PoolClient {
                    is_proxied: self.is_proxied,
                    tx: PoolTx::Http2(tx.clone()),
                };
                let a = PoolClient {
                    is_proxied: self.is_proxied,
                    tx: PoolTx::Http2(tx),
                };
                Reservation::Shared(a, b)
            }
        }
    }
}

// FIXME: allow() required due to `impl Trait` leaking types to this lint
#[allow(missing_debug_implementations)]
enum ClientError<B> {
    Normal(::Error),
    Canceled {
        connection_reused: bool,
        req: Request<B>,
        reason: ::Error,
    }
}

impl<B> ClientError<B> {
    fn map_with_reused(conn_reused: bool)
        -> impl Fn((::Error, Option<Request<B>>)) -> Self
    {
        move |(err, orig_req)| {
            if let Some(req) = orig_req {
                ClientError::Canceled {
                    connection_reused: conn_reused,
                    reason: err,
                    req,
                }
            } else {
                ClientError::Normal(err)
            }
        }
    }
}

/// A marker to identify what version a pooled connection is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Ver {
    Http1,
    Http2,
}

fn origin_form(uri: &mut Uri) {
    let path = match uri.path_and_query() {
        Some(path) if path.as_str() != "/" => {
            let mut parts = ::http::uri::Parts::default();
            parts.path_and_query = Some(path.clone());
            Uri::from_parts(parts).expect("path is valid uri")
        },
        _none_or_just_slash => {
            debug_assert!(Uri::default() == "/");
            Uri::default()
        }
    };
    *uri = path
}

fn absolute_form(uri: &mut Uri) {
    debug_assert!(uri.scheme_part().is_some(), "absolute_form needs a scheme");
    debug_assert!(uri.authority_part().is_some(), "absolute_form needs an authority");
    // If the URI is to HTTPS, and the connector claimed to be a proxy,
    // then it *should* have tunneled, and so we don't want to send
    // absolute-form in that case.
    if uri.scheme_part() == Some(&Scheme::HTTPS) {
        origin_form(uri);
    }
}

fn authority_form(uri: &mut Uri) {
    if log_enabled!(::log::Level::Warn) {
        if let Some(path) = uri.path_and_query() {
            // `https://hyper.rs` would parse with `/` path, don't
            // annoy people about that...
            if path != "/" {
                warn!(
                    "HTTP/1.1 CONNECT request stripping path: {:?}",
                    path
                );
            }
        }
    }
    *uri = match uri.authority_part() {
        Some(auth) => {
            let mut parts = ::http::uri::Parts::default();
            parts.authority = Some(auth.clone());
            Uri::from_parts(parts).expect("authority is valid")
        },
        None => {
            unreachable!("authority_form with relative uri");
        }
    };
}

fn set_scheme(uri: &mut Uri, scheme: Scheme) {
    debug_assert!(uri.scheme_part().is_none(), "set_scheme expects no existing scheme");
    let old = mem::replace(uri, Uri::default());
    let mut parts: ::http::uri::Parts = old.into();
    parts.scheme = Some(scheme);
    parts.path_and_query = Some("/".parse().expect("slash is a valid path"));
    *uri = Uri::from_parts(parts).expect("scheme is valid");
}

/// Builder for a Client
#[derive(Clone)]
pub struct Builder {
    //connect_timeout: Duration,
    exec: Exec,
    keep_alive: bool,
    keep_alive_timeout: Option<Duration>,
    h1_writev: bool,
    h1_title_case_headers: bool,
    max_idle_per_host: usize,
    retry_canceled_requests: bool,
    set_host: bool,
    ver: Ver,
}

impl Default for Builder {
    fn default() -> Self {
        Self {
            exec: Exec::Default,
            keep_alive: true,
            keep_alive_timeout: Some(Duration::from_secs(90)),
            h1_writev: true,
            h1_title_case_headers: false,
            max_idle_per_host: ::std::usize::MAX,
            retry_canceled_requests: true,
            set_host: true,
            ver: Ver::Http1,
        }
    }
}

impl Builder {
    /// Enable or disable keep-alive mechanics.
    ///
    /// Default is enabled.
    #[inline]
    pub fn keep_alive(&mut self, val: bool) -> &mut Self {
        self.keep_alive = val;
        self
    }

    /// Set an optional timeout for idle sockets being kept-alive.
    ///
    /// Pass `None` to disable timeout.
    ///
    /// Default is 90 seconds.
    #[inline]
    pub fn keep_alive_timeout<D>(&mut self, val: D) -> &mut Self
    where
        D: Into<Option<Duration>>,
    {
        self.keep_alive_timeout = val.into();
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
    pub fn http1_writev(&mut self, val: bool) -> &mut Self {
        self.h1_writev = val;
        self
    }

    /// Set whether HTTP/1 connections will write header names as title case at
    /// the socket level.
    ///
    /// Note that this setting does not affect HTTP/2.
    ///
    /// Default is false.
    pub fn http1_title_case_headers(&mut self, val: bool) -> &mut Self {
        self.h1_title_case_headers = val;
        self
    }

    /// Set whether the connection **must** use HTTP/2.
    ///
    /// Note that setting this to true prevents HTTP/1 from being allowed.
    ///
    /// Default is false.
    pub fn http2_only(&mut self, val: bool) -> &mut Self {
        self.ver = if val {
            Ver::Http2
        } else {
            Ver::Http1
        };
        self
    }

    /// Sets the maximum idle connection per host allowed in the pool.
    ///
    /// Default is `usize::MAX` (no limit).
    pub fn max_idle_per_host(&mut self, max_idle: usize) -> &mut Self {
        self.max_idle_per_host = max_idle;
        self
    }

    /// Set whether to retry requests that get disrupted before ever starting
    /// to write.
    ///
    /// This means a request that is queued, and gets given an idle, reused
    /// connection, and then encounters an error immediately as the idle
    /// connection was found to be unusable.
    ///
    /// When this is set to `false`, the related `ResponseFuture` would instead
    /// resolve to an `Error::Cancel`.
    ///
    /// Default is `true`.
    #[inline]
    pub fn retry_canceled_requests(&mut self, val: bool) -> &mut Self {
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
    pub fn set_host(&mut self, val: bool) -> &mut Self {
        self.set_host = val;
        self
    }

    /// Provide an executor to execute background `Connection` tasks.
    pub fn executor<E>(&mut self, exec: E) -> &mut Self
    where
        E: Executor<Box<Future<Item=(), Error=()> + Send>> + Send + Sync + 'static,
    {
        self.exec = Exec::Executor(Arc::new(exec));
        self
    }

    /// Builder a client with this configuration and the default `HttpConnector`.
    #[cfg(feature = "runtime")]
    pub fn build_http<B>(&self) -> Client<HttpConnector, B>
    where
        B: Payload + Send,
        B::Data: Send,
    {
        let mut connector = HttpConnector::new(4);
        if self.keep_alive {
            connector.set_keepalive(self.keep_alive_timeout);
        }
        self.build(connector)
    }

    /// Combine the configuration of this builder with a connector to create a `Client`.
    pub fn build<C, B>(&self, connector: C) -> Client<C, B>
    where
        C: Connect,
        C::Transport: 'static,
        C::Future: 'static,
        B: Payload + Send,
        B::Data: Send,
    {
        Client {
            connector: Arc::new(connector),
            executor: self.exec.clone(),
            h1_writev: self.h1_writev,
            h1_title_case_headers: self.h1_title_case_headers,
            pool: Pool::new(
                pool::Enabled(self.keep_alive),
                pool::IdleTimeout(self.keep_alive_timeout),
                pool::MaxIdlePerHost(self.max_idle_per_host),
                &self.exec,
            ),
            retry_canceled_requests: self.retry_canceled_requests,
            set_host: self.set_host,
            ver: self.ver,
        }
    }
}

impl fmt::Debug for Builder {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Builder")
            .field("keep_alive", &self.keep_alive)
            .field("keep_alive_timeout", &self.keep_alive_timeout)
            .field("http1_writev", &self.h1_writev)
            .field("max_idle_per_host", &self.max_idle_per_host)
            .field("set_host", &self.set_host)
            .field("version", &self.ver)
            .finish()
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn set_relative_uri_with_implicit_path() {
        let mut uri = "http://hyper.rs".parse().unwrap();
        origin_form(&mut uri);
        assert_eq!(uri.to_string(), "/");
    }

    #[test]
    fn test_origin_form() {
        let mut uri = "http://hyper.rs/guides".parse().unwrap();
        origin_form(&mut uri);
        assert_eq!(uri.to_string(), "/guides");

        let mut uri = "http://hyper.rs/guides?foo=bar".parse().unwrap();
        origin_form(&mut uri);
        assert_eq!(uri.to_string(), "/guides?foo=bar");
    }

    #[test]
    fn test_absolute_form() {
        let mut uri = "http://hyper.rs/guides".parse().unwrap();
        absolute_form(&mut uri);
        assert_eq!(uri.to_string(), "http://hyper.rs/guides");

        let mut uri = "https://hyper.rs/guides".parse().unwrap();
        absolute_form(&mut uri);
        assert_eq!(uri.to_string(), "/guides");
    }

    #[test]
    fn test_authority_form() {
        extern crate pretty_env_logger;
        let _ = pretty_env_logger::try_init();

        let mut uri = "http://hyper.rs".parse().unwrap();
        authority_form(&mut uri);
        assert_eq!(uri.to_string(), "hyper.rs");

        let mut uri = "hyper.rs".parse().unwrap();
        authority_form(&mut uri);
        assert_eq!(uri.to_string(), "hyper.rs");
    }
}
