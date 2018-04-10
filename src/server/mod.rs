//! HTTP Server
//!
//! A `Server` is created to listen on a port, parse HTTP requests, and hand
//! them off to a `Service`.

pub mod conn;
mod service;

use std::fmt;
use std::io;
use std::marker::PhantomData;
use std::net::{SocketAddr, TcpListener as StdTcpListener};
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use futures::task::{self, Task};
use futures::future::{self};
use futures::{Future, Stream, Poll, Async};
use futures_timer::Delay;
use http::{Request, Response};
use tokio_io::{AsyncRead, AsyncWrite};
use tokio::spawn;
use tokio::reactor::Handle;
use tokio::net::TcpListener;
pub use tokio_service::{NewService, Service};

use proto::body::{Body, Entity};
use proto;
use self::addr_stream::AddrStream;
use self::hyper_service::HyperService;

pub use self::conn::Connection;
pub use self::service::{const_service, service_fn};

/// A configuration of the HTTP protocol.
///
/// This structure is used to create instances of `Server` or to spawn off tasks
/// which handle a connection to an HTTP server. Each instance of `Http` can be
/// configured with various protocol-level options such as keepalive.
pub struct Http<B = ::Chunk> {
    max_buf_size: Option<usize>,
    keep_alive: bool,
    pipeline: bool,
    sleep_on_errors: bool,
    _marker: PhantomData<B>,
}

/// An instance of a server created through `Http::bind`.
///
/// This server is intended as a convenience for creating a TCP listener on an
/// address and then serving TCP connections accepted with the service provided.
pub struct Server<S, B>
where
    B: Entity,
{
    protocol: Http<B::Data>,
    new_service: S,
    handle: Handle,
    listener: TcpListener,
    shutdown_timeout: Duration,
}

/// A stream mapping incoming IOs to new services.
///
/// Yields `Connection`s that are futures that should be put on a reactor.
#[must_use = "streams do nothing unless polled"]
#[derive(Debug)]
pub struct Serve<I, S> {
    incoming: I,
    new_service: S,
    protocol: Http,
}

/*
#[must_use = "futures do nothing unless polled"]
#[derive(Debug)]
pub struct SpawnAll<I, S, E> {
    executor: E,
    serve: Serve<I, S>,
}
*/

/// A stream of connections from binding to an address.
#[must_use = "streams do nothing unless polled"]
pub struct AddrIncoming {
    addr: SocketAddr,
    keep_alive_timeout: Option<Duration>,
    listener: TcpListener,
    handle: Handle,
    sleep_on_errors: bool,
    timeout: Option<Delay>,
}

impl fmt::Debug for AddrIncoming {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("AddrIncoming")
            .field("addr", &self.addr)
            .field("keep_alive_timeout", &self.keep_alive_timeout)
            .field("listener", &self.listener)
            .field("handle", &self.handle)
            .field("sleep_on_errors", &self.sleep_on_errors)
            .finish()
    }
}

// ===== impl Http =====

impl<B: AsRef<[u8]> + 'static> Http<B> {
    /// Creates a new instance of the HTTP protocol, ready to spawn a server or
    /// start accepting connections.
    pub fn new() -> Http<B> {
        Http {
            keep_alive: true,
            max_buf_size: None,
            pipeline: false,
            sleep_on_errors: false,
            _marker: PhantomData,
        }
    }

    /// Enables or disables HTTP keep-alive.
    ///
    /// Default is true.
    pub fn keep_alive(&mut self, val: bool) -> &mut Self {
        self.keep_alive = val;
        self
    }

    /// Set the maximum buffer size for the connection.
    pub fn max_buf_size(&mut self, max: usize) -> &mut Self {
        self.max_buf_size = Some(max);
        self
    }

    /// Aggregates flushes to better support pipelined responses.
    ///
    /// Experimental, may be have bugs.
    ///
    /// Default is false.
    pub fn pipeline(&mut self, enabled: bool) -> &mut Self {
        self.pipeline = enabled;
        self
    }

    /// Swallow connection accept errors. Instead of passing up IO errors when
    /// the server is under heavy load the errors will be ignored. Some
    /// connection accept errors (like "connection reset") can be ignored, some
    /// (like "too many files open") may consume 100% CPU and a timout of 10ms
    /// is used in that case.
    ///
    /// Default is false.
    pub fn sleep_on_errors(&mut self, enabled: bool) -> &mut Self {
        self.sleep_on_errors = enabled;
        self
    }

    /// Bind the provided `addr` and return a server ready to handle
    /// connections.
    ///
    /// This method will bind the `addr` provided with a new TCP listener ready
    /// to accept connections. Each connection will be processed with the
    /// `new_service` object provided as well, creating a new service per
    /// connection.
    ///
    /// The returned `Server` contains one method, `run`, which is used to
    /// actually run the server.
    pub fn bind<S, Bd>(&self, addr: &SocketAddr, new_service: S) -> ::Result<Server<S, Bd>>
    where
        S: NewService<Request=Request<Body>, Response=Response<Bd>> + 'static,
        S::Error: Into<Box<::std::error::Error + Send + Sync>>,
        Bd: Entity<Data=B>,
    {
        let handle = Handle::current();
        let std_listener = StdTcpListener::bind(addr).map_err(::Error::new_listen)?;
        let listener = TcpListener::from_std(std_listener, &handle).map_err(::Error::new_listen)?;

        Ok(Server {
            new_service: new_service,
            handle: handle,
            listener: listener,
            protocol: self.clone(),
            shutdown_timeout: Duration::new(1, 0),
        })
    }

    /// Bind the provided `addr` and return a server with the default `Handle`.
    ///
    /// This is method will bind the `addr` provided with a new TCP listener ready
    /// to accept connections. Each connection will be processed with the
    /// `new_service` object provided as well, creating a new service per
    /// connection.
    pub fn serve_addr<S, Bd>(&self, addr: &SocketAddr, new_service: S) -> ::Result<Serve<AddrIncoming, S>>
    where
        S: NewService<Request=Request<Body>, Response=Response<Bd>>,
        S::Error: Into<Box<::std::error::Error + Send + Sync>>,
        Bd: Entity<Data=B>,
    {
        let handle = Handle::current();
        let std_listener = StdTcpListener::bind(addr).map_err(::Error::new_listen)?;
        let listener = TcpListener::from_std(std_listener, &handle).map_err(::Error::new_listen)?;
        let mut incoming = AddrIncoming::new(listener, handle.clone(), self.sleep_on_errors).map_err(::Error::new_listen)?;
        if self.keep_alive {
            incoming.set_keepalive(Some(Duration::from_secs(90)));
        }
        Ok(self.serve_incoming(incoming, new_service))
    }

    /// Bind the provided `addr` and return a server with a shared `Core`.
    ///
    /// This method allows the ability to share a `Core` with multiple servers.
    ///
    /// This is method will bind the `addr` provided with a new TCP listener ready
    /// to accept connections. Each connection will be processed with the
    /// `new_service` object provided as well, creating a new service per
    /// connection.
    pub fn serve_addr_handle<S, Bd>(&self, addr: &SocketAddr, handle: &Handle, new_service: S) -> ::Result<Serve<AddrIncoming, S>>
    where
        S: NewService<Request = Request<Body>, Response = Response<Bd>>,
        S::Error: Into<Box<::std::error::Error + Send + Sync>>,
        Bd: Entity<Data=B>,
    {
        let std_listener = StdTcpListener::bind(addr).map_err(::Error::new_listen)?;
        let listener = TcpListener::from_std(std_listener, &handle).map_err(::Error::new_listen)?;
        let mut incoming = AddrIncoming::new(listener, handle.clone(), self.sleep_on_errors).map_err(::Error::new_listen)?;

        if self.keep_alive {
            incoming.set_keepalive(Some(Duration::from_secs(90)));
        }
        Ok(self.serve_incoming(incoming, new_service))
    }

    /// Bind the provided stream of incoming IO objects with a `NewService`.
    ///
    /// This method allows the ability to share a `Core` with multiple servers.
    pub fn serve_incoming<I, S, Bd>(&self, incoming: I, new_service: S) -> Serve<I, S>
    where
        I: Stream<Error=::std::io::Error>,
        I::Item: AsyncRead + AsyncWrite,
        S: NewService<Request = Request<Body>, Response = Response<Bd>>,
        S::Error: Into<Box<::std::error::Error + Send + Sync>>,
        Bd: Entity<Data=B>,
    {
        Serve {
            incoming: incoming,
            new_service: new_service,
            protocol: Http {
                keep_alive: self.keep_alive,
                max_buf_size: self.max_buf_size,
                pipeline: self.pipeline,
                sleep_on_errors: self.sleep_on_errors,
                _marker: PhantomData,
            },
        }
    }

    /// Bind a connection together with a Service.
    ///
    /// This returns a Future that must be polled in order for HTTP to be
    /// driven on the connection.
    ///
    /// # Example
    ///
    /// ```
    /// # extern crate futures;
    /// # extern crate hyper;
    /// # extern crate tokio;
    /// # extern crate tokio_io;
    /// # use futures::Future;
    /// # use hyper::{Body, Request, Response};
    /// # use hyper::server::{Http, Service};
    /// # use tokio_io::{AsyncRead, AsyncWrite};
    /// # use tokio::reactor::Handle;
    /// # fn run<I, S>(some_io: I, some_service: S)
    /// # where
    /// #     I: AsyncRead + AsyncWrite + Send + 'static,
    /// #     S: Service<Request=Request<Body>, Response=Response<Body>, Error=hyper::Error> + Send + 'static,
    /// #     S::Future: Send
    /// # {
    /// let http = Http::<hyper::Chunk>::new();
    /// let conn = http.serve_connection(some_io, some_service);
    ///
    /// let fut = conn
    ///     .map(|_| ())
    ///     .map_err(|e| eprintln!("server connection error: {}", e));
    ///
    /// tokio::spawn(fut);
    /// # }
    /// # fn main() {}
    /// ```
    pub fn serve_connection<S, I, Bd>(&self, io: I, service: S) -> Connection<I, S>
    where
        S: Service<Request = Request<Body>, Response = Response<Bd>>,
        S::Error: Into<Box<::std::error::Error + Send + Sync>>,
        Bd: Entity,
        I: AsyncRead + AsyncWrite,
    {
        let mut conn = proto::Conn::new(io);
        if !self.keep_alive {
            conn.disable_keep_alive();
        }
        conn.set_flush_pipeline(self.pipeline);
        if let Some(max) = self.max_buf_size {
            conn.set_max_buf_size(max);
        }
        Connection {
            conn: proto::dispatch::Dispatcher::new(proto::dispatch::Server::new(service), conn),
        }
    }
}



impl<B> Clone for Http<B> {
    fn clone(&self) -> Http<B> {
        Http {
            ..*self
        }
    }
}

impl<B> fmt::Debug for Http<B> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Http")
            .field("keep_alive", &self.keep_alive)
            .field("pipeline", &self.pipeline)
            .finish()
    }
}



// ===== impl Server =====


/// TODO: add docs
pub struct Run(Box<Future<Item=(), Error=::Error> + Send + 'static>);

impl fmt::Debug for Run {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Run").finish()
    }
}

impl Future for Run {
    type Item = ();
    type Error = ::Error;

    fn poll(&mut self) -> Poll<(), ::Error> {
        self.0.poll()
    }
}


impl<S, B> Server<S, B>
where
    S: NewService<Request = Request<Body>, Response = Response<B>> + Send + 'static,
    S::Error: Into<Box<::std::error::Error + Send + Sync>>,
    <S as NewService>::Instance: Send,
    <<S as NewService>::Instance as Service>::Future: Send,
    B: Entity + Send + 'static,
    B::Data: Send,
{
    /// Returns the local address that this server is bound to.
    pub fn local_addr(&self) -> ::Result<SocketAddr> {
        //TODO: this shouldn't return an error at all, but should get the
        //local_addr at construction
        self.listener.local_addr().map_err(::Error::new_io)
    }

    /// Configure the amount of time this server will wait for a "graceful
    /// shutdown".
    ///
    /// This is the amount of time after the shutdown signal is received the
    /// server will wait for all pending connections to finish. If the timeout
    /// elapses then the server will be forcibly shut down.
    ///
    /// This defaults to 1s.
    pub fn shutdown_timeout(&mut self, timeout: Duration) -> &mut Self {
        self.shutdown_timeout = timeout;
        self
    }

    /// Execute this server infinitely.
    ///
    /// This method does not currently return, but it will return an error if
    /// one occurs.
    pub fn run(self) -> Run {
        self.run_until(future::empty())
    }

    /// Execute this server until the given future, `shutdown_signal`, resolves.
    ///
    /// This method, like `run` above, is used to execute this HTTP server. The
    /// difference with `run`, however, is that this method allows for shutdown
    /// in a graceful fashion. The future provided is interpreted as a signal to
    /// shut down the server when it resolves.
    ///
    /// This method will block the current thread executing the HTTP server.
    /// When the `shutdown_signal` has resolved then the TCP listener will be
    /// unbound (dropped). The thread will continue to block for a maximum of
    /// `shutdown_timeout` time waiting for active connections to shut down.
    /// Once the `shutdown_timeout` elapses or all active connections are
    /// cleaned out then this method will return.
    pub fn run_until<F>(self, shutdown_signal: F) -> Run
        where F: Future<Item = (), Error = ()> + Send + 'static,
    {
        let Server { protocol, new_service, handle, listener, shutdown_timeout } = self;

        let mut incoming = match AddrIncoming::new(listener, handle.clone(), protocol.sleep_on_errors) {
            Ok(incoming) => incoming,
            Err(err) => return Run(Box::new(future::err(::Error::new_listen(err)))),
        };

        if protocol.keep_alive {
            incoming.set_keepalive(Some(Duration::from_secs(90)));
        }

        // Mini future to track the number of active services
        let info = Arc::new(Mutex::new(Info {
            active: 0,
            blocker: None,
        }));

        // Future for our server's execution
        let info_cloned = info.clone();
        let srv = incoming.for_each(move |socket| {
            let addr = socket.remote_addr;
            debug!("accepted new connection ({})", addr);

            let service = new_service.new_service()?;
            let s = NotifyService {
                inner: service,
                info: Arc::downgrade(&info_cloned),
            };
            info_cloned.lock().unwrap().active += 1;
            let fut = protocol.serve_connection(socket, s)
                .map(|_| ())
                .map_err(move |err| error!("server connection error: ({}) {}", addr, err));
            spawn(fut);
            Ok(())
        });

        // for now, we don't care if the shutdown signal succeeds or errors
        // as long as it resolves, we will shutdown.
        let shutdown_signal = shutdown_signal.then(|_| Ok(()));

        // Main execution of the server. Here we use `select` to wait for either
        // `incoming` or `f` to resolve. We know that `incoming` will never
        // resolve with a success (it's infinite) so we're actually just waiting
        // for an error or for `f`, our shutdown signal.
        //
        // When we get a shutdown signal (`Ok`) then we drop the TCP listener to
        // stop accepting incoming connections.
        let main_execution = shutdown_signal.select(srv).then(move |result| {
            match result {
                Ok(((), _incoming)) => {},
                Err((e, _other)) => return future::Either::A(future::err(::Error::new_accept(e))),
            }

            // Ok we've stopped accepting new connections at this point, but we want
            // to give existing connections a chance to clear themselves out. Wait
            // at most `shutdown_timeout` time before we just return clearing
            // everything out.
            //
            // Our custom `WaitUntilZero` will resolve once all services constructed
            // here have been destroyed.
            let timeout = Delay::new(shutdown_timeout);
            let wait = WaitUntilZero { info: info.clone() };
            future::Either::B(wait.select(timeout).then(|result| {
                match result {
                    Ok(_) => Ok(()),
                    //TODO: error variant should be "timed out waiting for graceful shutdown"
                    Err((e, _)) => Err(::Error::new_io(e))
                }
            }))
        });

        Run(Box::new(main_execution))
    }
}

impl<S: fmt::Debug, B: Entity> fmt::Debug for Server<S, B>
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Server")
         .field("listener", &self.listener)
         .field("new_service", &self.new_service)
         .field("protocol", &self.protocol)
         .finish()
    }
}

// ===== impl Serve =====

impl<I, S> Serve<I, S> {
    /*
    /// Spawn all incoming connections onto the provide executor.
    pub fn spawn_all<E>(self, executor: E) -> SpawnAll<I, S, E> {
        SpawnAll {
            executor: executor,
            serve: self,
        }
    }
    */

    /// Get a reference to the incoming stream.
    #[inline]
    pub fn incoming_ref(&self) -> &I {
        &self.incoming
    }
}

impl<I, S, B> Stream for Serve<I, S>
where
    I: Stream<Error=io::Error>,
    I::Item: AsyncRead + AsyncWrite,
    S: NewService<Request=Request<Body>, Response=Response<B>>,
    S::Error: Into<Box<::std::error::Error + Send + Sync>>,
    B: Entity,
{
    type Item = Connection<I::Item, S::Instance>;
    type Error = ::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        if let Some(io) = try_ready!(self.incoming.poll().map_err(::Error::new_accept)) {
            let service = self.new_service.new_service().map_err(::Error::new_user_new_service)?;
            Ok(Async::Ready(Some(self.protocol.serve_connection(io, service))))
        } else {
            Ok(Async::Ready(None))
        }
    }
}

// ===== impl SpawnAll =====

/*
impl<I, S, E> Future for SpawnAll<I, S, E>
where
    I: Stream<Error=io::Error>,
    I::Item: AsyncRead + AsyncWrite,
    S: NewService<Request=Request<Body>, Response=Response<B>, Error=::Error>,
    B: Stream<Error=::Error>,
    B::Item: AsRef<[u8]>,
    //E: Executor<Connection<I::Item, S::Instance>>,
{
    type Item = ();
    type Error = ::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        loop {
            if let Some(conn) = try_ready!(self.serve.poll()) {
                let fut = conn
                    .map(|_| ())
                    .map_err(|err| debug!("conn error: {}", err));
                match self.executor.execute(fut) {
                    Ok(()) => (),
                    Err(err) => match err.kind() {
                        ExecuteErrorKind::NoCapacity => {
                            debug!("SpawnAll::poll; executor no capacity");
                            // continue loop
                        },
                        ExecuteErrorKind::Shutdown | _ => {
                            debug!("SpawnAll::poll; executor shutdown");
                            return Ok(Async::Ready(()))
                        }
                    }
                }
            } else {
                return Ok(Async::Ready(()))
            }
        }
    }
}
*/

// ===== impl AddrIncoming =====

impl AddrIncoming {
    fn new(listener: TcpListener, handle: Handle, sleep_on_errors: bool) -> io::Result<AddrIncoming> {
         Ok(AddrIncoming {
            addr: listener.local_addr()?,
            keep_alive_timeout: None,
            listener: listener,
            handle: handle,
            sleep_on_errors: sleep_on_errors,
            timeout: None,
        })
    }

    /// Get the local address bound to this listener.
    pub fn local_addr(&self) -> SocketAddr {
        self.addr
    }

    fn set_keepalive(&mut self, dur: Option<Duration>) {
        self.keep_alive_timeout = dur;
    }

    /*
    fn set_sleep_on_errors(&mut self, val: bool) {
        self.sleep_on_errors = val;
    }
    */
}

impl Stream for AddrIncoming {
    // currently unnameable...
    type Item = AddrStream;
    type Error = ::std::io::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        // Check if a previous timeout is active that was set by IO errors.
        if let Some(ref mut to) = self.timeout {
            match to.poll().expect("timeout never fails") {
                Async::Ready(_) => {}
                Async::NotReady => return Ok(Async::NotReady),
            }
        }
        self.timeout = None;
        loop {
            match self.listener.poll_accept() {
                Ok(Async::Ready((socket, addr))) => {
                    if let Some(dur) = self.keep_alive_timeout {
                        if let Err(e) = socket.set_keepalive(Some(dur)) {
                            trace!("error trying to set TCP keepalive: {}", e);
                        }
                    }
                    return Ok(Async::Ready(Some(AddrStream::new(socket, addr))));
                },
                Ok(Async::NotReady) => return Ok(Async::NotReady),
                Err(ref e) if self.sleep_on_errors => {
                    // Connection errors can be ignored directly, continue by
                    // accepting the next request.
                    if connection_error(e) {
                        continue;
                    }
                    // Sleep 10ms.
                    let delay = ::std::time::Duration::from_millis(10);
                    debug!("accept error: {}; sleeping {:?}",
                        e, delay);
                    let mut timeout = Delay::new(delay);
                    let result = timeout.poll()
                        .expect("timeout never fails");
                    match result {
                        Async::Ready(()) => continue,
                        Async::NotReady => {
                            self.timeout = Some(timeout);
                            return Ok(Async::NotReady);
                        }
                    }
                },
                Err(e) => return Err(e),
            }
        }
    }
}

/// This function defines errors that are per-connection. Which basically
/// means that if we get this error from `accept()` system call it means
/// next connection might be ready to be accepted.
///
/// All other errors will incur a timeout before next `accept()` is performed.
/// The timeout is useful to handle resource exhaustion errors like ENFILE
/// and EMFILE. Otherwise, could enter into tight loop.
fn connection_error(e: &io::Error) -> bool {
    e.kind() == io::ErrorKind::ConnectionRefused ||
    e.kind() == io::ErrorKind::ConnectionAborted ||
    e.kind() == io::ErrorKind::ConnectionReset
}

mod addr_stream {
    use std::io::{self, Read, Write};
    use std::net::SocketAddr;
    use bytes::{Buf, BufMut};
    use futures::Poll;
    use tokio::net::TcpStream;
    use tokio_io::{AsyncRead, AsyncWrite};


    #[derive(Debug)]
    pub struct AddrStream {
        inner: TcpStream,
        pub(super) remote_addr: SocketAddr,
    }

    impl AddrStream {
        pub(super) fn new(tcp: TcpStream, addr: SocketAddr) -> AddrStream {
            AddrStream {
                inner: tcp,
                remote_addr: addr,
            }
        }
    }

    impl Read for AddrStream {
        #[inline]
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            self.inner.read(buf)
        }
    }

    impl Write for AddrStream {
        #[inline]
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.inner.write(buf)
        }

        #[inline]
        fn flush(&mut self ) -> io::Result<()> {
            self.inner.flush()
        }
    }

    impl AsyncRead for AddrStream {
        #[inline]
        unsafe fn prepare_uninitialized_buffer(&self, buf: &mut [u8]) -> bool {
            self.inner.prepare_uninitialized_buffer(buf)
        }

        #[inline]
        fn read_buf<B: BufMut>(&mut self, buf: &mut B) -> Poll<usize, io::Error> {
            self.inner.read_buf(buf)
        }
    }

    impl AsyncWrite for AddrStream {
        #[inline]
        fn shutdown(&mut self) -> Poll<(), io::Error> {
            AsyncWrite::shutdown(&mut self.inner)
        }

        #[inline]
        fn write_buf<B: Buf>(&mut self, buf: &mut B) -> Poll<usize, io::Error> {
            self.inner.write_buf(buf)
        }
    }
}

// ===== NotifyService =====

struct NotifyService<S> {
    inner: S,
    info: Weak<Mutex<Info>>,
}

struct WaitUntilZero {
    info: Arc<Mutex<Info>>,
}

struct Info {
    active: usize,
    blocker: Option<Task>,
}

impl<S: Service> Service for NotifyService<S> {
    type Request = S::Request;
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn call(&self, message: Self::Request) -> Self::Future {
        self.inner.call(message)
    }
}

impl<S> Drop for NotifyService<S> {
    fn drop(&mut self) {
        let info = match self.info.upgrade() {
            Some(info) => info,
            None => return,
        };
        let mut info = info.lock().unwrap();
        info.active -= 1;
        if info.active == 0 {
            if let Some(task) = info.blocker.take() {
                task.notify();
            }
        }
    }
}

impl Future for WaitUntilZero {
    type Item = ();
    type Error = io::Error;

    fn poll(&mut self) -> Poll<(), io::Error> {
        let mut info = self.info.lock().unwrap();
        if info.active == 0 {
            Ok(().into())
        } else {
            info.blocker = Some(task::current());
            Ok(Async::NotReady)
        }
    }
}

mod hyper_service {
    use super::{Body, Entity, Request, Response, Service};
    /// A "trait alias" for any type that implements `Service` with hyper's
    /// Request, Response, and Error types, and a streaming body.
    ///
    /// There is an auto implementation inside hyper, so no one can actually
    /// implement this trait. It simply exists to reduce the amount of generics
    /// needed.
    pub trait HyperService: Service + Sealed {
        #[doc(hidden)]
        type ResponseBody;
        #[doc(hidden)]
        type Sealed: Sealed2;
    }

    pub trait Sealed {}
    pub trait Sealed2 {}

    #[allow(missing_debug_implementations)]
    pub struct Opaque {
        _inner: (),
    }

    impl Sealed2 for Opaque {}

    impl<S, B> Sealed for S
    where
        S: Service<
            Request=Request<Body>,
            Response=Response<B>,
        >,
        S::Error: Into<Box<::std::error::Error + Send + Sync>>,
        B: Entity,
    {}

    impl<S, B> HyperService for S
    where
        S: Service<
            Request=Request<Body>,
            Response=Response<B>,
        >,
        S::Error: Into<Box<::std::error::Error + Send + Sync>>,
        S: Sealed,
        B: Entity,
    {
        type ResponseBody = B;
        type Sealed = Opaque;
    }
}
