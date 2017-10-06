//! HTTP Server
//!
//! A `Server` is created to listen on a port, parse HTTP requests, and hand
//! them off to a `Service`.

#[cfg(feature = "compat")]
mod compat_impl;
#[cfg(feature = "compat")]
pub mod compat;

use std::cell::RefCell;
use std::fmt;
use std::io;
use std::marker::PhantomData;
use std::net::SocketAddr;
use std::rc::{Rc, Weak};
use std::time::Duration;

use futures::task::{self, Task};
use futures::future::{self, Select, Map};
use futures::{Future, Stream, Poll, Async, Sink, StartSend, AsyncSink};

#[cfg(feature = "compat")]
use http;

use tokio_io::{AsyncRead, AsyncWrite};
use tokio::reactor::{Core, Handle, Timeout};
use tokio::net::TcpListener;
use tokio_proto::BindServer;
use tokio_proto::streaming::Message;
use tokio_proto::streaming::pipeline::{Transport, Frame, ServerProto};
pub use tokio_service::{NewService, Service};

use proto;
use proto::response;
use proto::request;
#[cfg(feature = "compat")]
use proto::Body;

pub use proto::response::Response;
pub use proto::request::Request;

// The `Server` can be created use its own `Core`, or an shared `Handle`.
enum Reactor {
    // Own its `Core`
    Core(Core),
    // Share `Handle` with others
    Handle(Handle),
}

impl Reactor {
    /// Returns a handle to the underlying event loop that this server will be
    /// running on.
    #[inline]
    pub fn handle(&self) -> Handle {
        match *self {
            Reactor::Core(ref core) => core.handle(),
            Reactor::Handle(ref handle) => handle.clone(),
        }
    }
}

/// An instance of the HTTP protocol, and implementation of tokio-proto's
/// `ServerProto` trait.
///
/// This structure is used to create instances of `Server` or to spawn off tasks
/// which handle a connection to an HTTP server. Each instance of `Http` can be
/// configured with various protocol-level options such as keepalive.
pub struct Http<B = ::Chunk> {
    keep_alive: bool,
    pipeline: bool,
    _marker: PhantomData<B>,
}

/// An instance of a server created through `Http::bind`.
///
/// This server is intended as a convenience for creating a TCP listener on an
/// address and then serving TCP connections accepted with the service provided.
pub struct Server<S, B>
where B: Stream<Error=::Error>,
      B::Item: AsRef<[u8]>,
{
    protocol: Http<B::Item>,
    new_service: S,
    reactor: Reactor,
    listener: TcpListener,
    shutdown_timeout: Duration,
    no_proto: bool,
}

/// The Future of an Server.
pub struct ServerFuture<F, S, B>
where B: Stream<Error=::Error>,
      B::Item: AsRef<[u8]>,
{
    server: Server<S, B>,
    info: Rc<RefCell<Info>>,
    shutdown_signal: F,
    shutdown: Option<Select<WaitUntilZero, Timeout>>,
}

impl<B: AsRef<[u8]> + 'static> Http<B> {
    /// Creates a new instance of the HTTP protocol, ready to spawn a server or
    /// start accepting connections.
    pub fn new() -> Http<B> {
        Http {
            keep_alive: true,
            pipeline: false,
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

    /// Aggregates flushes to better support pipelined responses.
    ///
    /// Experimental, may be have bugs.
    ///
    /// Default is false.
    pub fn pipeline(&mut self, enabled: bool) -> &mut Self {
        self.pipeline = enabled;
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
        where S: NewService<Request = Request, Response = Response<Bd>, Error = ::Error> + 'static,
              Bd: Stream<Item=B, Error=::Error>,
    {
        let core = try!(Core::new());
        let handle = core.handle();
        let listener = try!(TcpListener::bind(addr, &handle));

        Ok(Server {
            new_service: new_service,
            reactor: Reactor::Core(core),
            listener: listener,
            protocol: self.clone(),
            shutdown_timeout: Duration::new(1, 0),
        })
    }

    /// This method allows the ability to share a `Core` with multiple servers.
    ///
    /// Bind the provided `addr` and return a server with a shared `Core`.
    ///
    /// This is method will bind the `addr` provided with a new TCP listener ready
    /// to accept connections. Each connection will be processed with the
    /// `new_service` object provided as well, creating a new service per
    /// connection.
    pub fn bind_handle<S, Bd>(&self, addr: &SocketAddr, new_service: S, handle: &Handle) -> ::Result<Server<S, Bd>>
        where S: NewService<Request = Request, Response = Response<Bd>, Error = ::Error> + 'static,
              Bd: Stream<Item=B, Error=::Error>,
    {
        let listener = TcpListener::bind(addr, &handle)?;

        Ok(Server {
            new_service: new_service,
            reactor: Reactor::Handle(handle.clone()),
            listener: listener,
            protocol: self.clone(),
            shutdown_timeout: Duration::new(1, 0),
            no_proto: false,
        })
    }

    /// Bind a `NewService` using types from the `http` crate.
    ///
    /// See `Http::bind`.
    #[cfg(feature = "compat")]
    pub fn bind_compat<S, Bd>(&self, addr: &SocketAddr, new_service: S) -> ::Result<Server<compat::NewCompatService<S>, Bd>>
        where S: NewService<Request = http::Request<Body>, Response = http::Response<Bd>, Error = ::Error> +
                    Send + Sync + 'static,
              Bd: Stream<Item=B, Error=::Error>,
    {
        self.bind(addr, self::compat_impl::new_service(new_service))
    }

    /// Use this `Http` instance to create a new server task which handles the
    /// connection `io` provided.
    ///
    /// This is the low-level method used to actually spawn handling a TCP
    /// connection, typically. The `handle` provided is the event loop on which
    /// the server task will be spawned, `io` is the I/O object associated with
    /// this connection (data that's read/written), `remote_addr` is the remote
    /// peer address of the HTTP client, and `service` defines how HTTP requests
    /// will be handled (and mapped to responses).
    ///
    /// This method is typically not invoked directly but is rather transitively
    /// used through [`bind`](#method.bind). This can be useful,
    /// however, when writing mocks or accepting sockets from a non-TCP
    /// location.
    pub fn bind_connection<S, I, Bd>(&self,
                                 handle: &Handle,
                                 io: I,
                                 remote_addr: SocketAddr,
                                 service: S)
        where S: Service<Request = Request, Response = Response<Bd>, Error = ::Error> + 'static,
              Bd: Stream<Item=B, Error=::Error> + 'static,
              I: AsyncRead + AsyncWrite + 'static,
    {
        self.bind_server(handle, io, HttpService {
            inner: service,
            remote_addr: remote_addr,
        })
    }

    /// Bind a connection together with a Service.
    ///
    /// This returns a Future that must be polled in order for HTTP to be
    /// driven on the connection.
    ///
    /// This additionally skips the tokio-proto infrastructure internally.
    pub fn no_proto<S, I, Bd>(&self, io: I, service: S) -> Connection<I, S>
        where S: Service<Request = Request, Response = Response<Bd>, Error = ::Error> + 'static,
              Bd: Stream<Item=B, Error=::Error> + 'static,
              I: AsyncRead + AsyncWrite + 'static,

    {
        let ka = if self.keep_alive {
            proto::KA::Busy
        } else {
            proto::KA::Disabled
        };
        let mut conn = proto::Conn::new(io, ka);
        conn.set_flush_pipeline(self.pipeline);
        Connection {
            conn: proto::dispatch::Dispatcher::new(proto::dispatch::Server::new(service), conn),
        }
    }

    /// Bind a `Service` using types from the `http` crate.
    ///
    /// See `Http::bind_connection`.
    #[cfg(feature = "compat")]
    pub fn bind_connection_compat<S, I, Bd>(&self,
                                 handle: &Handle,
                                 io: I,
                                 remote_addr: SocketAddr,
                                 service: S)
        where S: Service<Request = http::Request<Body>, Response = http::Response<Bd>, Error = ::Error> + 'static,
              Bd: Stream<Item=B, Error=::Error> + 'static,
              I: AsyncRead + AsyncWrite + 'static,
    {
        self.bind_server(handle, io, HttpService {
            inner: compat_impl::service(service),
            remote_addr: remote_addr,
        })
    }
}

use self::hyper_service::HyperService;
mod hyper_service {
    use super::{Request, Response, Service, Stream};
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
            Request=Request,
            Response=Response<B>,
            Error=::Error,
        >,
        B: Stream<Error=::Error>,
        B::Item: AsRef<[u8]>,
    {}

    impl<S, B> HyperService for S
    where
        S: Service<
            Request=Request,
            Response=Response<B>,
            Error=::Error,
        >,
        S: Sealed,
        B: Stream<Error=::Error>,
        B::Item: AsRef<[u8]>,
    {
        type ResponseBody = B;
        type Sealed = Opaque;
    }

}
/// A future binding a connection with a Service.
///
/// Polling this future will drive HTTP forward.
#[must_use = "futures do nothing unless polled"]
pub struct Connection<I, S>
where
    S: HyperService,
    S::ResponseBody: Stream<Error=::Error>,
    <S::ResponseBody as Stream>::Item: AsRef<[u8]>,
{
    conn: proto::dispatch::Dispatcher<proto::dispatch::Server<S>, S::ResponseBody, I, <S::ResponseBody as Stream>::Item, proto::ServerTransaction, proto::KA>,
}

impl<I, B, S> Future for Connection<I, S>
where S: Service<Request = Request, Response = Response<B>, Error = ::Error> + 'static,
      I: AsyncRead + AsyncWrite + 'static,
      B: Stream<Error=::Error> + 'static,
      B::Item: AsRef<[u8]>,
{
    type Item = self::unnameable::Opaque;
    type Error = ::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        try_ready!(self.conn.poll());
        Ok(self::unnameable::opaque().into())
    }
}

impl<I, S> fmt::Debug for Connection<I, S>
where
    S: HyperService,
    S::ResponseBody: Stream<Error=::Error>,
    <S::ResponseBody as Stream>::Item: AsRef<[u8]>,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Connection")
            .finish()
    }
}

mod unnameable {
    // This type is specifically not exported outside the crate,
    // so no one can actually name the type. With no methods, we make no
    // promises about this type.
    //
    // All of that to say we can eventually replace the type returned
    // to something else, and it would not be a breaking change.
    //
    // We may want to eventually yield the `T: AsyncRead + AsyncWrite`, which
    // doesn't have a `Debug` bound. So, this type can't implement `Debug`
    // either, so the type change doesn't break people.
    #[allow(missing_debug_implementations)]
    pub struct Opaque {
        _inner: (),
    }

    pub fn opaque() -> Opaque {
        Opaque {
            _inner: (),
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

#[doc(hidden)]
#[allow(missing_debug_implementations)]
pub struct __ProtoRequest(proto::RequestHead);
#[doc(hidden)]
#[allow(missing_debug_implementations)]
pub struct __ProtoResponse(proto::MessageHead<::StatusCode>);
#[doc(hidden)]
#[allow(missing_debug_implementations)]
pub struct __ProtoTransport<T, B>(proto::Conn<T, B, proto::ServerTransaction>);
#[doc(hidden)]
#[allow(missing_debug_implementations)]
pub struct __ProtoBindTransport<T, B> {
    inner: future::FutureResult<proto::Conn<T, B, proto::ServerTransaction>, io::Error>,
}

impl<T, B> ServerProto<T> for Http<B>
    where T: AsyncRead + AsyncWrite + 'static,
          B: AsRef<[u8]> + 'static,
{
    type Request = __ProtoRequest;
    type RequestBody = proto::Chunk;
    type Response = __ProtoResponse;
    type ResponseBody = B;
    type Error = ::Error;
    type Transport = __ProtoTransport<T, B>;
    type BindTransport = __ProtoBindTransport<T, B>;

    #[inline]
    fn bind_transport(&self, io: T) -> Self::BindTransport {
        let ka = if self.keep_alive {
            proto::KA::Busy
        } else {
            proto::KA::Disabled
        };
        let mut conn = proto::Conn::new(io, ka);
        conn.set_flush_pipeline(self.pipeline);
        __ProtoBindTransport {
            inner: future::ok(conn),
        }
    }
}

impl<T, B> Sink for __ProtoTransport<T, B>
    where T: AsyncRead + AsyncWrite + 'static,
          B: AsRef<[u8]> + 'static,
{
    type SinkItem = Frame<__ProtoResponse, B, ::Error>;
    type SinkError = io::Error;

    #[inline]
    fn start_send(&mut self, item: Self::SinkItem)
                  -> StartSend<Self::SinkItem, io::Error> {
        let item = match item {
            Frame::Message { message, body } => {
                Frame::Message { message: message.0, body: body }
            }
            Frame::Body { chunk } => Frame::Body { chunk: chunk },
            Frame::Error { error } => Frame::Error { error: error },
        };
        match try!(self.0.start_send(item)) {
            AsyncSink::Ready => Ok(AsyncSink::Ready),
            AsyncSink::NotReady(Frame::Message { message, body }) => {
                Ok(AsyncSink::NotReady(Frame::Message {
                    message: __ProtoResponse(message),
                    body: body,
                }))
            }
            AsyncSink::NotReady(Frame::Body { chunk }) => {
                Ok(AsyncSink::NotReady(Frame::Body { chunk: chunk }))
            }
            AsyncSink::NotReady(Frame::Error { error }) => {
                Ok(AsyncSink::NotReady(Frame::Error { error: error }))
            }
        }
    }

    #[inline]
    fn poll_complete(&mut self) -> Poll<(), io::Error> {
        self.0.poll_complete()
    }

    #[inline]
    fn close(&mut self) -> Poll<(), io::Error> {
        self.0.close()
    }
}

impl<T, B> Stream for __ProtoTransport<T, B>
    where T: AsyncRead + AsyncWrite + 'static,
          B: AsRef<[u8]> + 'static,
{
    type Item = Frame<__ProtoRequest, proto::Chunk, ::Error>;
    type Error = io::Error;

    #[inline]
    fn poll(&mut self) -> Poll<Option<Self::Item>, io::Error> {
        let item = match try_ready!(self.0.poll()) {
            Some(item) => item,
            None => return Ok(None.into()),
        };
        let item = match item {
            Frame::Message { message, body } => {
                Frame::Message { message: __ProtoRequest(message), body: body }
            }
            Frame::Body { chunk } => Frame::Body { chunk: chunk },
            Frame::Error { error } => Frame::Error { error: error },
        };
        Ok(Some(item).into())
    }
}

impl<T, B> Transport for __ProtoTransport<T, B>
    where T: AsyncRead + AsyncWrite + 'static,
          B: AsRef<[u8]> + 'static,
{
    #[inline]
    fn tick(&mut self) {
        self.0.tick()
    }

    #[inline]
    fn cancel(&mut self) -> io::Result<()> {
        self.0.cancel()
    }
}

impl<T, B> Future for __ProtoBindTransport<T, B>
    where T: AsyncRead + AsyncWrite + 'static,
{
    type Item = __ProtoTransport<T, B>;
    type Error = io::Error;

    #[inline]
    fn poll(&mut self) -> Poll<__ProtoTransport<T, B>, io::Error> {
        self.inner.poll().map(|a| a.map(__ProtoTransport))
    }
}

impl From<Message<__ProtoRequest, proto::TokioBody>> for Request {
    #[inline]
    fn from(message: Message<__ProtoRequest, proto::TokioBody>) -> Request {
        let (head, body) = match message {
            Message::WithoutBody(head) => (head.0, proto::Body::empty()),
            Message::WithBody(head, body) => (head.0, body.into()),
        };
        request::from_wire(None, head, body)
    }
}

impl<B> Into<Message<__ProtoResponse, B>> for Response<B> {
    #[inline]
    fn into(self) -> Message<__ProtoResponse, B> {
        let (head, body) = response::split(self);
        if let Some(body) = body {
            Message::WithBody(__ProtoResponse(head), body.into())
        } else {
            Message::WithoutBody(__ProtoResponse(head))
        }
    }
}

struct HttpService<T> {
    inner: T,
    remote_addr: SocketAddr,
}

impl<T, B> Service for HttpService<T>
    where T: Service<Request=Request, Response=Response<B>, Error=::Error>,
          B: Stream<Error=::Error>,
          B::Item: AsRef<[u8]>,
{
    type Request = Message<__ProtoRequest, proto::TokioBody>;
    type Response = Message<__ProtoResponse, B>;
    type Error = ::Error;
    type Future = Map<T::Future, fn(Response<B>) -> Message<__ProtoResponse, B>>;

    #[inline]
    fn call(&self, message: Self::Request) -> Self::Future {
        let (head, body) = match message {
            Message::WithoutBody(head) => (head.0, proto::Body::empty()),
            Message::WithBody(head, body) => (head.0, body.into()),
        };
        let req = request::from_wire(Some(self.remote_addr), head, body);
        self.inner.call(req).map(Into::into)
    }
}

impl<S, B> Server<S, B>
    where S: NewService<Request = Request, Response = Response<B>, Error = ::Error> + 'static,
          B: Stream<Error=::Error> + 'static,
          B::Item: AsRef<[u8]>,
{
    /// Returns the local address that this server is bound to.
    pub fn local_addr(&self) -> ::Result<SocketAddr> {
        Ok(try!(self.listener.local_addr()))
    }

    /// Returns a handle to the underlying event loop that this server will be
    /// running on.
    pub fn handle(&self) -> Handle {
        self.reactor.handle()
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

    /// Configure this server to not use tokio-proto infrastructure internally.
    pub fn no_proto(&mut self) -> &mut Self {
        self.no_proto = true;
        self
    }

    /// Configure the `shutdown_signal`.
    pub fn shutdown_signal<F>(self, signal: F) -> ServerFuture<F, S, B>
        where F: Future<Item = (), Error = ()>
    {
        ServerFuture {
            server: self,
            info: Rc::new(RefCell::new(Info {
                active: 0,
                blocker: None,
            })),
            shutdown_signal: signal,
            shutdown: None,
        }
    }

    /// Execute this server infinitely.
    ///
    /// This method does not currently return, but it will return an error if
    /// one occurs.
    pub fn run(self) -> ::Result<()> {
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
    pub fn run_until<F>(self, shutdown_signal: F) -> ::Result<()>
        where F: Future<Item = (), Error = ()>,
    {
        let Server { protocol, new_service, reactor, listener, shutdown_timeout, no_proto } = self;

        let mut core = match reactor {
            Reactor::Core(core) => core,
            _ => panic!("Server does not own its core, use `Handle::spawn()` to run the service!"),
        };

        let handle = core.handle();

        // Mini future to track the number of active services
        let info = Rc::new(RefCell::new(Info {
            active: 0,
            blocker: None,
        }));

        // Future for our server's execution
        let srv = listener.incoming().for_each(|(socket, addr)| {
            let s = NotifyService {
                inner: try!(new_service.new_service()),
                info: Rc::downgrade(&info),
            };
            info.borrow_mut().active += 1;
            if no_proto {
                let fut = protocol.no_proto(socket, s)
                    .map(|_| ())
                    .map_err(|err| error!("no_proto error: {}", err));
                handle.spawn(fut);
            } else {
                protocol.bind_connection(&handle, socket, addr, s);
            }
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
        match core.run(shutdown_signal.select(srv)) {
            Ok(((), _incoming)) => {}
            Err((e, _other)) => return Err(e.into()),
        }

        // Ok we've stopped accepting new connections at this point, but we want
        // to give existing connections a chance to clear themselves out. Wait
        // at most `shutdown_timeout` time before we just return clearing
        // everything out.
        //
        // Our custom `WaitUntilZero` will resolve once all services constructed
        // here have been destroyed.
        let timeout = try!(Timeout::new(shutdown_timeout, &handle));
        let wait = WaitUntilZero { info: info.clone() };
        match core.run(wait.select(timeout)) {
            Ok(_) => Ok(()),
            Err((e, _)) => Err(e.into())
        }
    }
}

impl<S, B> Future for Server<S, B>
    where S: NewService<Request = Request, Response = Response<B>, Error = ::Error> + 'static,
          B: Stream<Error=::Error> + 'static,
          B::Item: AsRef<[u8]>,
{
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        if let Reactor::Core(_) = self.reactor {
            panic!("Server owns its core, use `Server::run()` to run the service!")
        }

        loop {
            match self.listener.accept() {
                Ok((socket, addr)) => {
                    // TODO: use the NotifyService
                    match self.new_service.new_service() {
                        Ok(srv) => self.protocol.bind_connection(&self.handle(),
                                                                 socket,
                                                                 addr,
                                                                 srv),
                        Err(e) => debug!("internal error: {:?}", e),
                    }
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => return Ok(Async::NotReady),
                Err(e) => debug!("internal error: {:?}", e),
            }
        }
    }
}

impl<F, S, B> Future for ServerFuture<F, S, B>
    where F: Future<Item = (), Error = ()>,
          S: NewService<Request = Request, Response = Response<B>, Error = ::Error> + 'static,
          B: Stream<Error=::Error> + 'static,
          B::Item: AsRef<[u8]>,
{
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        loop {
            if let Some(ref mut shutdown) = self.shutdown {
                match shutdown.poll() {
                    Ok(Async::Ready(_)) => return Ok(Async::Ready(())),
                    Ok(Async::NotReady) => return Ok(Async::NotReady),
                    Err((e, _)) => debug!("internal error: {:?}", e),
                }
            } else if let Ok(Async::Ready(())) = self.shutdown_signal.poll() {
                 match Timeout::new(self.server.shutdown_timeout, &self.server.handle()) {
                    Ok(timeout) => {
                        let wait = WaitUntilZero { info: self.info.clone() };
                        self.shutdown = Some(wait.select(timeout))
                    },
                    Err(e) => debug!("internal error: {:?}", e),
                }
            } else {
                match self.server.listener.accept() {
                    Ok((socket, addr)) => {
                        match self.server.new_service.new_service() {
                            Ok(inner_srv) => {
                                let srv = NotifyService {
                                    inner: inner_srv,
                                    info: Rc::downgrade(&self.info),
                                };
                                self.info.borrow_mut().active += 1;
                                self.server.protocol.bind_connection(&self.server.handle(),
                                                                     socket,
                                                                     addr,
                                                                     srv)
                            },
                            Err(e) => debug!("internal error: {:?}", e),
                        }
                    },
                    Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => return Ok(Async::NotReady),
                    Err(e) => debug!("internal error: {:?}", e),
                }
            }
        }
    }
}


impl<S: fmt::Debug, B: Stream<Error=::Error>> fmt::Debug for Server<S, B>
where B::Item: AsRef<[u8]>
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Server")
         .field("reactor", &"...")
         .field("listener", &self.listener)
         .field("new_service", &self.new_service)
         .field("protocol", &self.protocol)
         .finish()
    }
}

impl <F, S: fmt::Debug, B: Stream<Error=::Error>> fmt::Debug for ServerFuture<F, S, B>
where B::Item: AsRef<[u8]>,
F: Future<Item = (), Error = ()>
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("ServerFuture")
         .field("server", &self.server)
         .field("info", &"...")
         .field("shutdown_signal", &"...")
         .field("shutdown", &"...")
         .finish()
    }
}

struct NotifyService<S> {
    inner: S,
    info: Weak<RefCell<Info>>,
}

struct WaitUntilZero {
    info: Rc<RefCell<Info>>,
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
        let mut info = info.borrow_mut();
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
        let mut info = self.info.borrow_mut();
        if info.active == 0 {
            Ok(().into())
        } else {
            info.blocker = Some(task::current());
            Ok(Async::NotReady)
        }
    }
}
