//! HTTP Server
//!
//! A `Server` is created to listen on a port, parse HTTP requests, and hand
//! them off to a `Service`.

use std::cell::RefCell;
use std::fmt;
use std::io;
use std::marker::PhantomData;
use std::net::SocketAddr;
use std::rc::{Rc, Weak};
use std::time::Duration;

use futures::future;
use futures::task::{self, Task};
use futures::{Future, Stream, Poll, Async, Sink, StartSend, AsyncSink};
use futures::future::{Map, Empty, Select};
use futures::stream::{FromErr};

use tokio_io::{AsyncRead, AsyncWrite};
use tokio::reactor::{Core, Handle, Timeout};
use tokio::net::{TcpListener, TcpStream, Incoming};
use tokio_proto::BindServer;
use tokio_proto::streaming::Message;
use tokio_proto::streaming::pipeline::{Transport, Frame, ServerProto};
pub use tokio_service::{NewService, Service};


use http;
use http::response;
use http::request;

pub use http::response::Response;
pub use http::request::Request;

/// An instance of the HTTP protocol, and implementation of tokio-proto's
/// `ServerProto` trait.
///
/// This structure is used to create instances of `Server` or to spawn off tasks
/// which handle a connection to an HTTP server. Each instance of `Http` can be
/// configured with various protocol-level options such as keepalive.
pub struct Http<B = ::Chunk> {
    keep_alive: bool,
    _marker: PhantomData<B>,
}

/// An instance of a server created through `Http::bind`.
///
/// This server is intended as a convenience for creating a TCP listener on an
/// address and then serving TCP connections accepted with the service provided.
pub struct Server<S, B, IO, Listener>
where B: Stream<Error=::Error>,
      B::Item: AsRef<[u8]>,
      IO: AsyncRead + AsyncWrite,
      Listener: Stream<Item = (IO, SocketAddr), Error = ::Error>
{
    protocol: Http<B::Item>,
    new_service: S,
    listener: Listener,
    shutdown_timeout: Duration,
}

impl<B: AsRef<[u8]> + 'static> Http<B> {
    /// Creates a new instance of the HTTP protocol, ready to spawn a server or
    /// start accepting connections.
    pub fn new() -> Http<B> {
        Http {
            keep_alive: true,
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
    pub fn bind<S, Bd>(&self, addr: &SocketAddr, handle: Handle, new_service: S) -> ::Result<Server<S, Bd, TcpStream, FromErr<Incoming, ::Error>>>
        where S: NewService<Request = Request, Response = Response<Bd>, Error = ::Error> +
                    'static,
              Bd: Stream<Item=B, Error=::Error>,
    {
        Ok(self.from_listener(
            try!(TcpListener::bind(addr, &handle)).incoming().from_err(),
            new_service
        ))
    }

    /// Use the provided `listener` as a stream of connections.
    ///
    /// This method is used for example by [`bind`](#method.bind),
    /// using `tokio_io::net::Incoming` as parameter `listener`.
    ///
    /// The returned `Server` contains one method, `run`, which is
    /// used to actually run the server.
    pub fn from_listener<S, Bd, IO, Listener>(&self, listener: Listener, new_service: S) -> Server<S, Bd, IO, Listener>
        where S: NewService<Request = Request, Response = Response<Bd>, Error = ::Error> +
                    'static,
              Bd: Stream<Item=B, Error=::Error>,
              IO: AsyncRead + AsyncWrite,
              Listener: Stream<Item = (IO, SocketAddr), Error = ::Error>
    {
        Server {
            new_service,
            listener,
            protocol: self.clone(),
            shutdown_timeout: Duration::new(1, 0),
        }
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
            .finish()
    }
}

#[doc(hidden)]
#[allow(missing_debug_implementations)]
pub struct __ProtoRequest(http::RequestHead);
#[doc(hidden)]
#[allow(missing_debug_implementations)]
pub struct __ProtoResponse(ResponseHead);
#[doc(hidden)]
#[allow(missing_debug_implementations)]
pub struct __ProtoTransport<T, B>(http::Conn<T, B, http::ServerTransaction>);
#[doc(hidden)]
#[allow(missing_debug_implementations)]
pub struct __ProtoBindTransport<T, B> {
    inner: future::FutureResult<http::Conn<T, B, http::ServerTransaction>, io::Error>,
}

impl<T, B> ServerProto<T> for Http<B>
    where T: AsyncRead + AsyncWrite + 'static,
          B: AsRef<[u8]> + 'static,
{
    type Request = __ProtoRequest;
    type RequestBody = http::Chunk;
    type Response = __ProtoResponse;
    type ResponseBody = B;
    type Error = ::Error;
    type Transport = __ProtoTransport<T, B>;
    type BindTransport = __ProtoBindTransport<T, B>;

    #[inline]
    fn bind_transport(&self, io: T) -> Self::BindTransport {
        let ka = if self.keep_alive {
            http::KA::Busy
        } else {
            http::KA::Disabled
        };
        __ProtoBindTransport {
            inner: future::ok(http::Conn::new(io, ka)),
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
    type Item = Frame<__ProtoRequest, http::Chunk, ::Error>;
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

impl From<Message<__ProtoRequest, http::TokioBody>> for Request {
    #[inline]
    fn from(message: Message<__ProtoRequest, http::TokioBody>) -> Request {
        let (head, body) = match message {
            Message::WithoutBody(head) => (head.0, http::Body::empty()),
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

type ResponseHead = http::MessageHead<::StatusCode>;

impl<T, B> Service for HttpService<T>
    where T: Service<Request=Request, Response=Response<B>, Error=::Error>,
          B: Stream<Error=::Error>,
          B::Item: AsRef<[u8]>,
{
    type Request = Message<__ProtoRequest, http::TokioBody>;
    type Response = Message<__ProtoResponse, B>;
    type Error = ::Error;
    type Future = Map<T::Future, fn(Response<B>) -> Message<__ProtoResponse, B>>;

    #[inline]
    fn call(&self, message: Self::Request) -> Self::Future {
        let (head, body) = match message {
            Message::WithoutBody(head) => (head.0, http::Body::empty()),
            Message::WithBody(head, body) => (head.0, body.into()),
        };
        let req = request::from_wire(Some(self.remote_addr), head, body);
        self.inner.call(req).map(Into::into)
    }
}

impl<S, B, I, L> Server<S, B, I, L>
    where S: NewService<Request = Request, Response = Response<B>, Error = ::Error>
             + 'static,
          B: Stream<Error=::Error> + 'static,
          B::Item: AsRef<[u8]>,
          I: AsyncRead + AsyncWrite + 'static,
          L: Stream<Item = (I, SocketAddr), Error = ::Error>
{

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
        let mut core = Core::new()?;
        let handle = core.handle();
        core.run(self.future_run_until(handle, shutdown_signal))
    }

    /// Build a future that executes this server infinitely.
    ///
    /// The returned `RunUntil` is meant to be run by, or spawned into
    /// an event loop, like so:
    ///
    /// ```
    /// let mut core = Core::new().unwrap();
    /// let handle = core.handle();
    /// core.run(self.future_run_until(handle, shutdown_signal)).unwrap()
    /// ```
    pub fn future_run(self, handle: Handle) -> RunUntil<Empty<(), ()>, S, B, I, L> {
        self.future_run_until(handle, future::empty())
    }


    /// Build a future that runs the server until the given future,
    /// `shutdown_signal`, resolves.
    ///
    /// This method, like `future_run` above, is used to execute this
    /// HTTP server. The difference with `future_run`, however, is
    /// that this method allows for shutdown in a graceful
    /// fashion. The future provided is interpreted as a signal to
    /// shut down the server when it resolves.
    ///
    /// When the `shutdown_signal` has resolved then the TCP listener
    /// will be unbound (dropped). The future will resolve after a
    /// maximum of `shutdown_timeout` time waiting for active
    /// connections to shut down.  Once the `shutdown_timeout` elapses
    /// or all active connections are cleaned out then the future will
    /// resolve to `()`.
    pub fn future_run_until<F>(self, handle: Handle, shutdown_signal: F) -> RunUntil<F, S, B, I, L>
        where F: Future<Item = (), Error = ()>,
    {
        RunUntil {
            server: self,
            shutdown_signal,
            info: Rc::new(RefCell::new(Info {
                active: 0,
                blocker: None,
            })),
            handle,
            shutdown: None,
        }
    }
}

/// A future that runs a HTTP server. See the
/// [`future_run`](./struct.Server.html#method.future_run) and
/// [`future_run_until`](./struct.Server.html#method.future_run_until)
/// methods of [`Http`](./struct.Server.html) to understand how to
/// build instances of this.
pub struct RunUntil<F, S, B, I, L>
    where B: Stream<Error=::Error> + 'static,
          B::Item: AsRef<[u8]>,
          I: AsyncRead + AsyncWrite,
          L: Stream<Item = (I, SocketAddr), Error = ::Error>
{
    server: Server<S, B, I, L>,
    info: Rc<RefCell<Info>>,
    shutdown_signal: F,
    handle: Handle,
    shutdown: Option<Select<WaitUntilZero, Timeout>>,
}

impl<F, S, B, I, L> Future for RunUntil<F, S, B, I, L>
    where F:Future<Item = (), Error = ()>,
          S: NewService<Request = Request, Response = Response<B>, Error = ::Error>
                + 'static,
          B: Stream<Error=::Error> + 'static,
          B::Item: AsRef<[u8]>,
          I: AsyncRead + AsyncWrite + 'static,
          L: Stream<Item = (I, SocketAddr), Error = ::Error> {

    type Item = ();
    type Error = ::Error;
    fn poll(&mut self) -> Poll<(), ::Error> {

        loop {
            if let Some(ref mut shutdown) = self.shutdown {
                match shutdown.poll() {
                    Ok(Async::Ready(_)) => return Ok(Async::Ready(())),
                    Ok(Async::NotReady) => return Ok(Async::NotReady),
                    Err((e, _)) => return Err(e.into())
                }
            } else if let Ok(Async::Ready(())) = self.shutdown_signal.poll() {
                let timeout = Timeout::new(self.server.shutdown_timeout, &self.handle)?;
                let wait = WaitUntilZero { info: self.info.clone() };
                self.shutdown = Some(wait.select(timeout))
            } else {
                match self.server.listener.poll() {
                    Ok(Async::Ready(Some((socket, addr)))) => {
                        let s = NotifyService {
                            inner: try!(self.server.new_service.new_service()),
                            info: Rc::downgrade(&self.info),
                        };
                        self.info.borrow_mut().active += 1;
                        self.server.protocol.bind_connection(&self.handle, socket, addr, s);
                    }
                    Ok(Async::Ready(None)) => return Ok(Async::Ready(())),
                    Ok(Async::NotReady) => return Ok(Async::NotReady),
                    Err(e) => debug!("hyper error: {:?}", e)
                }
            }
        }
    }
}



impl<S: fmt::Debug, B: Stream<Error=::Error>, I: AsyncRead+AsyncWrite, L:Stream<Item=(I, SocketAddr), Error=::Error> + fmt::Debug> fmt::Debug for Server<S, B, I, L>
where B::Item: AsRef<[u8]>
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Server")
         .field("core", &"...")
         .field("listener", &self.listener)
         .field("new_service", &self.new_service)
         .field("protocol", &self.protocol)
         .finish()
    }
}

impl<F, S: fmt::Debug, B: Stream<Error=::Error>, I: AsyncRead+AsyncWrite, L:Stream<Item=(I, SocketAddr), Error=::Error> + fmt::Debug> fmt::Debug for RunUntil<F, S, B, I, L>
where B::Item: AsRef<[u8]>
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("RunUntil")
         .field("server", &self.server)
         .field("handle", &self.handle)
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
