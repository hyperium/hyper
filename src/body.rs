//! Streaming bodies for Requests and Responses
//!
//! For both [Clients](::client) and [Servers](::server), requests and
//! responses use streaming bodies, instead of complete buffering. This
//! allows applications to not use memory they don't need, and allows exerting
//! back-pressure on connections by only reading when asked.
//!
//! There are two pieces to this in hyper:
//!
//! - The [`Payload`](Payload) trait the describes all possible bodies. hyper
//!   allows any body type that implements `Payload`, allowing applications to
//!   have fine-grained control over their streaming.
//! - The [`Body`](Body) concrete type, which is an implementation of `Payload`,
//!  and returned by hyper as a "receive stream" (so, for server requests and
//!  client responses). It is also a decent default implementation if you don't
//!  have very custom needs of your send streams.
use std::borrow::Cow;
use std::fmt;

use bytes::Bytes;
use futures::{Async, Future, Poll, Stream};
use futures::sync::{mpsc, oneshot};
use h2;
use http::HeaderMap;

use common::Never;
pub use chunk::Chunk;

type BodySender = mpsc::Sender<Result<Chunk, ::Error>>;

/// This trait represents a streaming body of a `Request` or `Response`.
///
/// The built-in implementation of this trait is [`Body`](Body), in case you
/// don't need to customize a send stream for your own application.
pub trait Payload: Send + 'static {
    /// A buffer of bytes representing a single chunk of a body.
    type Data: AsRef<[u8]> + Send;

    /// The error type of this stream.
    type Error: Into<Box<::std::error::Error + Send + Sync>>;

    /// Poll for a `Data` buffer.
    ///
    /// Similar to `Stream::poll_next`, this yields `Some(Data)` until
    /// the body ends, when it yields `None`.
    fn poll_data(&mut self) -> Poll<Option<Self::Data>, Self::Error>;

    /// Poll for an optional **single** `HeaderMap` of trailers.
    ///
    /// This should **only** be called after `poll_data` has ended.
    ///
    /// Note: Trailers aren't currently used for HTTP/1, only for HTTP/2.
    fn poll_trailers(&mut self) -> Poll<Option<HeaderMap>, Self::Error> {
        Ok(Async::Ready(None))
    }

    /// A hint that the `Body` is complete, and doesn't need to be polled more.
    ///
    /// This can be useful to determine if the there is any body or trailers
    /// without having to poll. An empty `Body` could return `true` and hyper
    /// would be able to know that only the headers need to be sent. Or, it can
    /// also be checked after each `poll_data` call, to allow hyper to try to
    /// end the underlying stream with the last chunk, instead of needing to
    /// send an extra `DATA` frame just to mark the stream as finished.
    ///
    /// As a hint, it is used to try to optimize, and thus is OK for a default
    /// implementation to return `false`.
    fn is_end_stream(&self) -> bool {
        false
    }

    /// Return a length of the total bytes that will be streamed, if known.
    ///
    /// If an exact size of bytes is known, this would allow hyper to send a
    /// `Content-Length` header automatically, not needing to fall back to
    /// `Transfer-Encoding: chunked`.
    ///
    /// This does not need to be kept updated after polls, it will only be
    /// called once to create the headers.
    fn content_length(&self) -> Option<u64> {
        None
    }
}

impl<E: Payload> Payload for Box<E> {
    type Data = E::Data;
    type Error = E::Error;

    fn poll_data(&mut self) -> Poll<Option<Self::Data>, Self::Error> {
        (**self).poll_data()
    }

    fn poll_trailers(&mut self) -> Poll<Option<HeaderMap>, Self::Error> {
        (**self).poll_trailers()
    }

    fn is_end_stream(&self) -> bool {
        (**self).is_end_stream()
    }

    fn content_length(&self) -> Option<u64> {
        (**self).content_length()
    }
}


/// A stream of `Chunk`s, used when receiving bodies.
///
/// A good default `Payload` to use in many applications.
///
/// Also implements `futures::Stream`, so stream combinators may be used.
#[must_use = "streams do nothing unless polled"]
pub struct Body {
    kind: Kind,
    /// Allow the client to pass a future to delay the `Body` from returning
    /// EOF. This allows the `Client` to try to put the idle connection
    /// back into the pool before the body is "finished".
    ///
    /// The reason for this is so that creating a new request after finishing
    /// streaming the body of a response could sometimes result in creating
    /// a brand new connection, since the pool didn't know about the idle
    /// connection yet.
    delayed_eof: Option<DelayEof>,
}

enum Kind {
    Chan {
        _close_tx: oneshot::Sender<()>,
        rx: mpsc::Receiver<Result<Chunk, ::Error>>,
    },
    H2(h2::RecvStream),
    Wrapped(Box<Stream<Item=Chunk, Error=Box<::std::error::Error + Send + Sync>> + Send>),
    Once(Option<Chunk>),
    Empty,
}

type DelayEofUntil = oneshot::Receiver<Never>;

enum DelayEof {
    /// Initial state, stream hasn't seen EOF yet.
    NotEof(DelayEofUntil),
    /// Transitions to this state once we've seen `poll` try to
    /// return EOF (`None`). This future is then polled, and
    /// when it completes, the Body finally returns EOF (`None`).
    Eof(DelayEofUntil),
}

/// A sender half used with `Body::channel()`.
///
/// Useful when wanting to stream chunks from another thread. See
/// [`Body::channel`](Body::channel) for more.
#[must_use = "Sender does nothing unless sent on"]
#[derive(Debug)]
pub struct Sender {
    close_rx: oneshot::Receiver<()>,
    tx: BodySender,
}

impl Body {
    /// Create an empty `Body` stream.
    ///
    /// # Example
    ///
    /// ```
    /// use hyper::{Body, Request};
    ///
    /// // create a `GET /` request
    /// let get = Request::new(Body::empty());
    /// ```
    #[inline]
    pub fn empty() -> Body {
        Body::new(Kind::Empty)
    }

    /// Create a `Body` stream with an associated sender half.
    ///
    /// Useful when wanting to stream chunks from another thread.
    #[inline]
    pub fn channel() -> (Sender, Body) {
        let (tx, rx) = mpsc::channel(0);
        let (close_tx, close_rx) = oneshot::channel();

        let tx = Sender {
            close_rx: close_rx,
            tx: tx,
        };
        let rx = Body::new(Kind::Chan {
            _close_tx: close_tx,
            rx: rx,
        });

        (tx, rx)
    }

    /// Wrap a futures `Stream` in a box inside `Body`.
    ///
    /// # Example
    ///
    /// ```
    /// # extern crate futures;
    /// # extern crate hyper;
    /// # use hyper::Body;
    /// # fn main() {
    /// let chunks = vec![
    ///     "hello",
    ///     " ",
    ///     "world",
    /// ];
    ///
    /// let stream = futures::stream::iter_ok::<_, ::std::io::Error>(chunks);
    ///
    /// let body = Body::wrap_stream(stream);
    /// # }
    /// ```
    pub fn wrap_stream<S>(stream: S) -> Body
    where
        S: Stream + Send + 'static,
        S::Error: Into<Box<::std::error::Error + Send + Sync>>,
        Chunk: From<S::Item>,
    {
        let mapped = stream
            .map(Chunk::from)
            .map_err(Into::into);
        Body::new(Kind::Wrapped(Box::new(mapped)))
    }

    /// Returns if this body was constructed via `Body::empty()`.
    ///
    /// # Note
    ///
    /// This does **not** detect if the body stream may be at the end, or
    /// if the stream will not yield any chunks, in all cases. For instance,
    /// a streaming body using `chunked` encoding is not able to tell if
    /// there are more chunks immediately.
    ///
    /// See [`is_end_stream`](Payload::is_end_stream) for a dynamic version.
    #[inline]
    pub fn is_empty(&self) -> bool {
        match self.kind {
            Kind::Empty => true,
            _ => false,
        }
    }

    fn new(kind: Kind) -> Body {
        Body {
            kind: kind,
            delayed_eof: None,
        }
    }

    pub(crate) fn h2(recv: h2::RecvStream) -> Self {
        Body::new(Kind::H2(recv))
    }

    pub(crate) fn delayed_eof(&mut self, fut: DelayEofUntil) {
        self.delayed_eof = Some(DelayEof::NotEof(fut));
    }

    fn poll_eof(&mut self) -> Poll<Option<Chunk>, ::Error> {
        match self.delayed_eof.take() {
            Some(DelayEof::NotEof(mut delay)) => {
                match self.poll_inner() {
                    ok @ Ok(Async::Ready(Some(..))) |
                    ok @ Ok(Async::NotReady) => {
                        self.delayed_eof = Some(DelayEof::NotEof(delay));
                        ok
                    },
                    Ok(Async::Ready(None)) => match delay.poll() {
                        Ok(Async::Ready(never)) => match never {},
                        Ok(Async::NotReady) => {
                            self.delayed_eof = Some(DelayEof::Eof(delay));
                            Ok(Async::NotReady)
                        },
                        Err(_done) => {
                            Ok(Async::Ready(None))
                        },
                    },
                    Err(e) => Err(e),
                }
            },
            Some(DelayEof::Eof(mut delay)) => {
                match delay.poll() {
                    Ok(Async::Ready(never)) => match never {},
                    Ok(Async::NotReady) => {
                        self.delayed_eof = Some(DelayEof::Eof(delay));
                        Ok(Async::NotReady)
                    },
                    Err(_done) => {
                        Ok(Async::Ready(None))
                    },
                }
            },
            None => self.poll_inner(),
        }
    }

    fn poll_inner(&mut self) -> Poll<Option<Chunk>, ::Error> {
        match self.kind {
            Kind::Chan { ref mut rx, .. } => match rx.poll().expect("mpsc cannot error") {
                Async::Ready(Some(Ok(chunk))) => Ok(Async::Ready(Some(chunk))),
                Async::Ready(Some(Err(err))) => Err(err),
                Async::Ready(None) => Ok(Async::Ready(None)),
                Async::NotReady => Ok(Async::NotReady),
            },
            Kind::H2(ref mut h2) => {
                h2.poll()
                    .map(|async| {
                        async.map(|opt| {
                            opt.map(|bytes| {
                                Chunk::h2(bytes, h2.release_capacity())
                            })
                        })
                    })
                    .map_err(::Error::new_body)
            },
            Kind::Wrapped(ref mut s) => s.poll().map_err(::Error::new_body),
            Kind::Once(ref mut val) => Ok(Async::Ready(val.take())),
            Kind::Empty => Ok(Async::Ready(None)),
        }
    }
}

impl Default for Body {
    /// Returns [`Body::empty()`](Body::empty).
    #[inline]
    fn default() -> Body {
        Body::empty()
    }
}

impl Payload for Body {
    type Data = Chunk;
    type Error = ::Error;

    fn poll_data(&mut self) -> Poll<Option<Self::Data>, Self::Error> {
        self.poll_eof()
    }

    fn poll_trailers(&mut self) -> Poll<Option<HeaderMap>, Self::Error> {
        match self.kind {
            Kind::H2(ref mut h2) => h2.poll_trailers().map_err(::Error::new_h2),
            _ => Ok(Async::Ready(None)),
        }
    }

    fn is_end_stream(&self) -> bool {
        match self.kind {
            Kind::Chan { .. } => false,
            Kind::H2(ref h2) => h2.is_end_stream(),
            Kind::Wrapped(..) => false,
            Kind::Once(ref val) => val.is_none(),
            Kind::Empty => true
        }
    }

    fn content_length(&self) -> Option<u64> {
        match self.kind {
            Kind::Chan { .. } => None,
            Kind::H2(..) => None,
            Kind::Wrapped(..) => None,
            Kind::Once(Some(ref val)) => Some(val.len() as u64),
            Kind::Once(None) => None,
            Kind::Empty => Some(0)
        }
    }
}

impl Stream for Body {
    type Item = Chunk;
    type Error = ::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        self.poll_data()
    }
}

impl fmt::Debug for Body {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Body")
            .finish()
    }
}

impl Sender {
    /// Check to see if this `Sender` can send more data.
    pub fn poll_ready(&mut self) -> Poll<(), ::Error> {
        match self.close_rx.poll() {
            Ok(Async::Ready(())) | Err(_) => return Err(::Error::new_closed()),
            Ok(Async::NotReady) => (),
        }

        self.tx.poll_ready().map_err(|_| ::Error::new_closed())
    }

    /// Sends data on this channel.
    ///
    /// This should be called after `poll_ready` indicated the channel
    /// could accept another `Chunk`.
    ///
    /// Returns `Err(Chunk)` if the channel could not (currently) accept
    /// another `Chunk`.
    pub fn send_data(&mut self, chunk: Chunk) -> Result<(), Chunk> {
        self.tx.try_send(Ok(chunk))
            .map_err(|err| err.into_inner().expect("just sent Ok"))
    }

    pub(crate) fn send_error(&mut self, err: ::Error) {
        let _ = self.tx.try_send(Err(err));
    }
}

impl From<Chunk> for Body {
    #[inline]
    fn from(chunk: Chunk) -> Body {
        if chunk.is_empty() {
            Body::empty()
        } else {
            Body::new(Kind::Once(Some(chunk)))
        }
    }
}

impl From<Bytes> for Body {
    #[inline]
    fn from (bytes: Bytes) -> Body {
        Body::from(Chunk::from(bytes))
    }
}

impl From<Vec<u8>> for Body {
    #[inline]
    fn from (vec: Vec<u8>) -> Body {
        Body::from(Chunk::from(vec))
    }
}

impl From<&'static [u8]> for Body {
    #[inline]
    fn from (slice: &'static [u8]) -> Body {
        Body::from(Chunk::from(slice))
    }
}

impl From<Cow<'static, [u8]>> for Body {
    #[inline]
    fn from (cow: Cow<'static, [u8]>) -> Body {
        match cow {
            Cow::Borrowed(b) => Body::from(b),
            Cow::Owned(o) => Body::from(o)
        }
    }
}

impl From<String> for Body {
    #[inline]
    fn from (s: String) -> Body {
        Body::from(Chunk::from(s.into_bytes()))
    }
}

impl From<&'static str> for Body {
    #[inline]
    fn from(slice: &'static str) -> Body {
        Body::from(Chunk::from(slice.as_bytes()))
    }
}

impl From<Cow<'static, str>> for Body {
    #[inline]
    fn from(cow: Cow<'static, str>) -> Body {
        match cow {
            Cow::Borrowed(b) => Body::from(b),
            Cow::Owned(o) => Body::from(o)
        }
    }
}

fn _assert_send_sync() {
    fn _assert_send<T: Send>() {}
    fn _assert_sync<T: Sync>() {}

    _assert_send::<Body>();
    _assert_send::<Chunk>();
    _assert_sync::<Chunk>();
}

#[test]
fn test_body_stream_concat() {
    use futures::{Stream, Future};

    let body = Body::from("hello world");

    let total = body
        .concat2()
        .wait()
        .unwrap();
    assert_eq!(total.as_ref(), b"hello world");
}

