//! The tokio-proto `ServerProto` machinery.
//!
//! Not to be confused with `hyper::proto`.
//!
//! Will be deprecated soon.

use std::io;
use std::net::SocketAddr;

#[cfg(feature = "compat")]
use http;
use futures::future::{self, Map};
use futures::{Future, Stream, Poll, Sink, StartSend, AsyncSink};
use tokio::reactor::Handle;
use tokio_io::{AsyncRead, AsyncWrite};
use tokio_proto::BindServer;
use tokio_proto::streaming::Message;
use tokio_proto::streaming::pipeline::{Transport, Frame, ServerProto};
use tokio_service::Service;

use {Request, Response};
use proto::{self, request, response};
#[cfg(feature = "compat")]
use proto::Body;
#[cfg(feature = "compat")]
use super::compat;
use super::Http;

impl<B: AsRef<[u8]> + 'static> Http<B> {
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
            inner: compat::service(service),
            remote_addr: remote_addr,
        })
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
        if let Some(max) = self.max_buf_size {
            conn.set_max_buf_size(max);
        }
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
            Message::WithoutBody(head) => (head.0, None),
            Message::WithBody(head, body) => (head.0, Some(body.into())),
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
            Message::WithoutBody(head) => (head.0, None),
            Message::WithBody(head, body) => (head.0, Some(body.into())),
        };
        let req = request::from_wire(Some(self.remote_addr), head, body);
        self.inner.call(req).map(Into::into)
    }
}
