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
use futures::future::Map;
use futures::sync::oneshot::{self, Receiver, Sender};

use tokio_io::{AsyncRead, AsyncWrite};
use tokio::reactor::{Core, Handle, Timeout};
use tokio::net::TcpListener;
use tokio_proto::BindServer;
use tokio_proto::streaming::Message;
use tokio_proto::streaming::pipeline::{Transport, Frame, ServerProto};
pub use tokio_service::{NewService, Service};

use bytes::BytesMut;

use http;
use http::response;
use http::request;

pub use http::response::Response;
pub use http::request::Request;

use common::Omitted;

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
pub struct Server<S, B>
where B: Stream<Error=::Error>,
      B::Item: AsRef<[u8]>,
{
    protocol: Http<B::Item>,
    new_service: S,
    core: Core,
    listener: TcpListener,
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
    pub fn bind<S, Bd>(&self, addr: &SocketAddr, new_service: S) -> ::Result<Server<S, Bd>>
        where S: NewService<Request = Request, Response = Response<Bd>, Error = ::Error> +
                    Send + Sync + 'static,
              Bd: Stream<Item=B, Error=::Error>,
    {
        let core = try!(Core::new());
        let handle = core.handle();
        let listener = try!(TcpListener::bind(addr, &handle));

        Ok(Server {
            new_service: new_service,
            core: core,
            listener: listener,
            protocol: self.clone(),
            shutdown_timeout: Duration::new(1, 0),
        })
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

    /// Use this `Http` instance to create a new server task which handles the
    /// connection `io` provided, and can be upgraded to another protocol in
    /// response to a request from the client.
    ///
    /// This behaves like `bind_connection`, except the `Service` supplied must
    /// return an `UpgradableResponse` containing either an HTTP `Response` or
    /// some information describing a protocol switch. If an `Upgrade` response
    /// is returned, the server task is shut down (without closing the
    /// underlying `io`) and this information is forwarded to the `Future`
    /// returned by this method, along with the underlying `io` and any
    /// remaining buffered data that has already been read from it.
    ///
    /// If the server task shuts down normally, the `Future` completes with a
    /// `None` output value. In the case of an error handling the connection,
    /// the error is also forwarded to this `Future`.
    pub fn bind_upgradable_connection<S, I, P, Bd>(&self,
                                                   handle: &Handle,
                                                   io: I,
                                                   remote_addr: SocketAddr,
                                                   service: S)
                                                   -> BindUpgradableConnection<I, B, P>
        where S: Service<Request = Request,
                         Response = UpgradableResponse<P, Bd>,
                         Error = ::Error> + 'static,
              Bd: Stream<Item=B, Error=::Error> + 'static,
              I: AsyncRead + AsyncWrite + 'static,
              P: 'static,
    {
        let (sender, receiver) = oneshot::channel();
        let proto = UpgradableHttp {
            inner: self.clone(),
            _marker: PhantomData,
        };
        proto.bind_server(handle, (io, sender), UpgradableHttpService {
            inner: service,
            remote_addr: remote_addr,
        });
        BindUpgradableConnection {
            receiver: receiver,
            state: None,
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

impl<B, P> Into<Message<UpgradableResponseHead<P>, B>> for UpgradableResponse<P, B> {
    #[inline]
    fn into(self) -> Message<UpgradableResponseHead<P>, B> {
        match self {
            UpgradableResponse::Response(res) => {
                let (head, body) = response::split(res);
                if let Some(body) = body {
                    Message::WithBody(Ok(__ProtoResponse(head)), body.into())
                } else {
                    Message::WithoutBody(Ok(__ProtoResponse(head)))
                }
            }
            UpgradableResponse::Upgrade(upgrade_info, maybe_head) => {
                Message::WithoutBody(Err((upgrade_info, maybe_head.map(|head| {
                    __ProtoResponse(response::split(head).0)
                }))))
            }
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

struct UpgradableHttp<P, B = ::Chunk> {
    inner: Http<B>,
    _marker: PhantomData<P>,
}

type UpgradablePayload<T, B, P> = Result<(__ProtoTransport<T, B>, P), ::Error>;
type UpgradableReceiver<T, B, P> = Receiver<UpgradablePayload<T, B, P>>;
type UpgradableSender<T, B, P> = Sender<UpgradablePayload<T, B, P>>;

type UpgradableResponseHead<P> = Result<__ProtoResponse, (P, Option<__ProtoResponse>)>;

impl<T, B, P> ServerProto<(T, UpgradableSender<T, B, P>)> for UpgradableHttp<P, B>
    where T: AsyncRead + AsyncWrite + 'static,
          B: AsRef<[u8]> + 'static,
          P: 'static,
{
    type Request = __ProtoRequest;
    type RequestBody = http::Chunk;
    type Response = UpgradableResponseHead<P>;
    type ResponseBody = B;
    type Error = ::Error;
    type Transport = UpgradableTransport<T, B, P>;
    type BindTransport = UpgradableBindTransport<T, B, P>;

    #[inline]
    fn bind_transport(&self, (io, sender): (T, UpgradableSender<T, B, P>)) -> Self::BindTransport {
        UpgradableBindTransport {
            inner: self.inner.bind_transport(io),
            sender: Some(sender),
        }
    }
}

struct UpgradableBindTransport<T, B, P> {
    inner: __ProtoBindTransport<T, B>,
    sender: Option<UpgradableSender<T, B, P>>,
}

impl<T, B, P> Future for UpgradableBindTransport<T, B, P>
    where T: AsyncRead + AsyncWrite + 'static,
{
    type Item = UpgradableTransport<T, B, P>;
    type Error = io::Error;

    #[inline]
    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let inner_transport = try_ready!(self.inner.poll());
        let transport = UpgradableTransport {
            inner: Some(inner_transport),
            sender: Some(self.sender.take().unwrap()),
            recv_task: None,
        };
        Ok(transport.into())
    }
}

struct UpgradableTransport<T, B, P> {
    inner: Option<__ProtoTransport<T, B>>,
    sender: Option<UpgradableSender<T, B, P>>,
    recv_task: Option<Task>,
}

impl<T, B, P> UpgradableTransport<T, B, P> {
    fn inner_mut(&mut self) -> &mut __ProtoTransport<T, B> {
        self.inner.as_mut().expect("hyper: UpgradableTransport missing inner")
    }

    fn take_inner(&mut self) -> __ProtoTransport<T, B> {
        self.inner.take().expect("hyper: UpgradableTransport missing inner")
    }

    fn take_sender(&mut self) -> UpgradableSender<T, B, P> {
        self.sender.take().expect("hyper: UpgradableTransport missing sender")
    }

    fn send_error<E>(&mut self, err: E) -> E
        where E: From<io::Error> + Into<::Error>,
    {
        match self.sender.take() {
            None => err,
            Some(sender) => {
                let _ = sender.send(Err(err.into()));
                proto_error()
            }
        }
    }
}

/// This error will go back to the tokio-proto internals which in turn will
/// simply discard it and shut down the connection, so we don't need to worry
/// about what it contains.
fn proto_error<E>() -> E
    where E: From<io::Error>
{
    io::Error::from(io::ErrorKind::Other).into()
}

impl<T, B, P> Stream for UpgradableTransport<T, B, P>
    where T: AsyncRead + AsyncWrite + 'static,
          B: AsRef<[u8]> + 'static,
{
    type Item = Frame<__ProtoRequest, http::Chunk, ::Error>;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        let result = match self.inner {
            None => return Ok(None.into()),
            Some(ref mut inner) => {
                self.recv_task = Some(task::current());
                inner.poll()
            }
        };
        let item = match try_ready!(result.map_err(|err| self.send_error(err))) {
            Some(Frame::Error { error }) => Some(Frame::Error { error: self.send_error(error) }),
            item => item,
        };
        Ok(item.into())
    }
}

impl<T, B, P> Sink for UpgradableTransport<T, B, P>
    where T: AsyncRead + AsyncWrite + 'static,
          B: AsRef<[u8]> + 'static,
{
    type SinkItem = Frame<UpgradableResponseHead<P>, B, ::Error>;
    type SinkError = io::Error;

    fn start_send(&mut self, item: Self::SinkItem) -> StartSend<Self::SinkItem, Self::SinkError> {
        let item = match item {
            Frame::Error { error } => {
                let alt = proto_error();
                let result = self.inner_mut().start_send(Frame::Error { error: alt });
                return Ok(match result.map_err(|err| self.send_error(err))? {
                    AsyncSink::NotReady(_) => AsyncSink::NotReady(Frame::Error { error: error }),
                    AsyncSink::Ready => {
                        self.send_error(error);
                        AsyncSink::Ready
                    }
                });
            }
            Frame::Body { chunk } => Frame::Body { chunk: chunk },
            Frame::Message { message, body } => {
                match message {
                    Ok(message) => Frame::Message { message: message, body: body },
                    Err((upgrade_info, maybe_head)) => {
                        if let Some(head) = maybe_head {
                            let result = self.inner_mut().start_send(Frame::Message {
                                message: head,
                                body: false
                            });
                            match result.map_err(|err| self.send_error(err))? {
                                AsyncSink::Ready => (),
                                AsyncSink::NotReady(item) => {
                                    match item {
                                        Frame::Message { message, body } => {
                                            return Ok(AsyncSink::NotReady(Frame::Message {
                                                message: Err((upgrade_info, Some(message))),
                                                body: body,
                                            }));
                                        }
                                        _ => unreachable!(),
                                    }
                                }
                            }
                        }

                        let inner = self.take_inner();
                        let sender = self.take_sender();
                        let _ = sender.send(Ok((inner, upgrade_info)));

                        if let Some(recv_task) = self.recv_task.take() {
                            recv_task.notify();
                        }

                        return Ok(AsyncSink::Ready);
                    }
                }
            }
        };

        let result = self.inner_mut().start_send(item);
        Ok(match result.map_err(|err| self.send_error(err))? {
            AsyncSink::Ready => AsyncSink::Ready,
            AsyncSink::NotReady(item) => {
                AsyncSink::NotReady(match item {
                    Frame::Message { message, body } => {
                        Frame::Message {
                            message: Ok(message),
                            body: body,
                        }
                    }
                    Frame::Body { chunk } => Frame::Body { chunk: chunk },
                    Frame::Error {..} => unreachable!(),
                })
            }
        })
    }

    fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
        let result = match self.inner {
            None => return Ok(().into()),
            Some(ref mut inner) => inner.poll_complete(),
        };
        result.map_err(|err| self.send_error(err))
    }

    fn close(&mut self) -> Poll<(), Self::SinkError> {
        let result = match self.inner {
            None => return Ok(().into()),
            Some(ref mut inner) => inner.close(),
        };
        result.map_err(|err| self.send_error(err))
    }
}

impl<T, B, P> Transport for UpgradableTransport<T, B, P>
    where T: AsyncRead + AsyncWrite + 'static,
          B: AsRef<[u8]> + 'static,
          P: 'static,
{
    #[inline]
    fn tick(&mut self) {
        if let Some(ref mut inner) = self.inner {
            inner.tick();
        }
    }

    #[inline]
    fn cancel(&mut self) -> io::Result<()> {
        if self.recv_task.as_mut().map_or(false, |task| task.will_notify_current()) {
            self.recv_task = None;
        }

        if let Some(ref mut inner) = self.inner {
            inner.cancel()
        } else {
            Ok(())
        }
    }
}

/// The return type of `Http::bind_upgradable_connection`.
///
/// If the server task shuts down normally, this future completes with a `None`
/// output value.
///
/// If an `Upgrade` response is returned by the bound `Service`, the server task
/// is shut down (without closing the underlying I/O object) and the provided
/// upgrade information is forwarded to this future, along with the underlying
/// I/O object and any remaining buffered data that has already been read from
/// it.
///
/// In the case of an error handling the connection, this future will complete
/// with that error.
pub struct BindUpgradableConnection<T, B, P> {
    receiver: UpgradableReceiver<T, B, P>,
    state: Option<(http::Conn<T, B, http::ServerTransaction>, P)>,
}

impl<T, B, P> fmt::Debug for BindUpgradableConnection<T, B, P>
    where B: AsRef<[u8]>,
          P: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("BindUpgradableConnection")
            .field("receiver", &Omitted)
            .field("state", &self.state)
            .finish()
    }
}

impl<T, B, P> Future for BindUpgradableConnection<T, B, P>
    where T: AsyncRead + AsyncWrite + 'static,
          B: AsRef<[u8]> + 'static,
{
    type Item = Option<(T, BytesMut, P)>;
    type Error = ::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        loop {
            if let Some((ref mut conn, _)) = self.state {
                try_ready!(conn.poll_complete());
            }

            if let Some((conn, upgrade_info)) = self.state.take() {
                // We ensure above that the connection is flushed before
                // unwrapping.
                let (io, read_buf) = conn.try_into_inner().ok().unwrap();
                return Ok(Some((io, read_buf, upgrade_info)).into());
            }

            match self.receiver.poll() {
                Err(_) => return Ok(None.into()),
                Ok(Async::NotReady) => return Ok(Async::NotReady),
                Ok(Async::Ready(result)) => {
                    match result {
                        Err(err) => return Err(err),
                        Ok((transport, upgrade_info)) => {
                            self.state = Some((transport.0, upgrade_info));
                        }
                    }
                }
            }
        }
    }
}

/// Either an HTTP response or a signal to upgrade from HTTP to another
/// protocol.
///
/// If the latter, the `Upgrade` variant should contain information pertaining
/// to this protocol switch.
pub enum UpgradableResponse<P = (), B = ::Body> {
    /// An HTTP response.
    Response(Response<B>),
    /// A protocol upgrade signal with accompanying information and optional
    /// "switching protocols" HTTP response head.
    Upgrade(P, Option<Response<()>>),
}

impl<P, B> fmt::Debug for UpgradableResponse<P, B>
    where Response<B>: fmt::Debug,
          P: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            UpgradableResponse::Response(ref response) => {
                f.debug_tuple("Response").field(response).finish()
            }
            UpgradableResponse::Upgrade(ref upgrade_info, ref response) => {
                f.debug_tuple("Upgrade").field(upgrade_info).field(response).finish()
            }
        }
    }
}

impl<P, B> From<Response<B>> for UpgradableResponse<P, B> {
    fn from(src: Response<B>) -> Self {
        UpgradableResponse::Response(src)
    }
}

impl<P, B> Default for UpgradableResponse<P, B> {
    fn default() -> Self {
        Response::default().into()
    }
}

struct UpgradableHttpService<T> {
    inner: T,
    remote_addr: SocketAddr,
}

impl<T, B, P> Service for UpgradableHttpService<T>
    where T: Service<Request=Request, Response=UpgradableResponse<P, B>, Error=::Error>,
          B: Stream<Error=::Error>,
          B::Item: AsRef<[u8]>,
{
    type Request = Message<__ProtoRequest, http::TokioBody>;
    type Response = Message<UpgradableResponseHead<P>, B>;
    type Error = ::Error;
    type Future = Map<T::Future,
                      fn(UpgradableResponse<P, B>) -> Message<UpgradableResponseHead<P>, B>>;

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

impl<S, B> Server<S, B>
    where S: NewService<Request = Request, Response = Response<B>, Error = ::Error>
                + Send + Sync + 'static,
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
        self.core.handle()
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
        let Server { protocol, new_service, mut core, listener, shutdown_timeout } = self;
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
            protocol.bind_connection(&handle, socket, addr, s);
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

impl<S: fmt::Debug, B: Stream<Error=::Error>> fmt::Debug for Server<S, B>
where B::Item: AsRef<[u8]>
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Server")
         .field("core", &Omitted)
         .field("listener", &self.listener)
         .field("new_service", &self.new_service)
         .field("protocol", &self.protocol)
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
