//! Streaming bodies for Requests and Responses
use std::borrow::Cow;
use std::fmt;

use bytes::Bytes;
use futures::{Async, Future, Never, Poll, Stream, StreamExt};
use futures::task;
use futures::channel::{mpsc, oneshot};
use http::HeaderMap;

use super::Chunk;

type BodySender = mpsc::Sender<Result<Chunk, ::Error>>;

/// This trait represents a streaming body of a `Request` or `Response`.
pub trait Entity {
    /// A buffer of bytes representing a single chunk of a body.
    type Data: AsRef<[u8]>;

    /// The error type of this stream.
    //TODO: add bounds Into<::error::User> (or whatever it is called)
    type Error;

    /// Poll for a `Data` buffer.
    ///
    /// Similar to `Stream::poll_next`, this yields `Some(Data)` until
    /// the body ends, when it yields `None`.
    fn poll_data(&mut self, cx: &mut task::Context) -> Poll<Option<Self::Data>, Self::Error>;

    /// Poll for an optional **single** `HeaderMap` of trailers.
    ///
    /// This should **only** be called after `poll_data` has ended.
    ///
    /// Note: Trailers aren't currently used for HTTP/1, only for HTTP/2.
    fn poll_trailers(&mut self, _cx: &mut task::Context) -> Poll<Option<HeaderMap>, Self::Error> {
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

impl<E: Entity> Entity for Box<E> {
    type Data = E::Data;
    type Error = E::Error;

    fn poll_data(&mut self, cx: &mut task::Context) -> Poll<Option<Self::Data>, Self::Error> {
        (**self).poll_data(cx)
    }

    fn poll_trailers(&mut self, cx: &mut task::Context) -> Poll<Option<HeaderMap>, Self::Error> {
        (**self).poll_trailers(cx)
    }

    fn is_end_stream(&self) -> bool {
        (**self).is_end_stream()
    }

    fn content_length(&self) -> Option<u64> {
        (**self).content_length()
    }
}

/// A wrapper to consume an `Entity` as a futures `Stream`.
#[must_use = "streams do nothing unless polled"]
#[derive(Debug)]
pub struct EntityStream<E> {
    is_data_eof: bool,
    entity: E,
}

impl<E: Entity> Stream for EntityStream<E> {
    type Item = E::Data;
    type Error = E::Error;

    fn poll_next(&mut self, cx: &mut task::Context) -> Poll<Option<Self::Item>, Self::Error> {
        loop {
            if self.is_data_eof {
                return self.entity.poll_trailers(cx)
                    .map(|async| {
                        async.map(|_opt| {
                            // drop the trailers and return that Stream is done
                            None
                        })
                    });
            }

            let opt = try_ready!(self.entity.poll_data(cx));
            if let Some(data) = opt {
                return Ok(Async::Ready(Some(data)));
            } else {
                self.is_data_eof = true;
            }
        }
    }
}

/// An `Entity` of `Chunk`s, used when receiving bodies.
///
/// Also a good default `Entity` to use in many applications.
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
    Wrapped(Box<Stream<Item=Chunk, Error=::Error> + Send>),
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
    /// let stream = futures::stream::iter_ok(chunks);
    ///
    /// let body = Body::wrap_stream(stream);
    /// # }
    /// ```
    pub fn wrap_stream<S>(stream: S) -> Body
    where
        S: Stream<Error=::Error> + Send + 'static,
        Chunk: From<S::Item>,
    {
        Body::new(Kind::Wrapped(Box::new(stream.map(Chunk::from))))
    }

    /// Convert this `Body` into a `Stream<Item=Chunk, Error=hyper::Error>`.
    ///
    /// # Example
    ///
    /// ```
    /// # extern crate futures;
    /// # extern crate hyper;
    /// # use futures::{FutureExt, StreamExt};
    /// # use hyper::{Body, Request};
    /// # fn request_concat(some_req: Request<Body>) {
    /// let req: Request<Body> = some_req;
    /// let body = req.into_body();
    ///
    /// let stream = body.into_stream();
    /// stream.concat()
    ///     .map(|buf| {
    ///         println!("body length: {}", buf.len());
    ///     });
    /// # }
    /// # fn main() {}
    /// ```
    #[inline]
    pub fn into_stream(self) -> EntityStream<Body> {
        EntityStream {
            is_data_eof: false,
            entity: self,
        }
    }

    /// Returns if this body was constructed via `Body::empty()`.
    ///
    /// # Note
    ///
    /// This does **not** detect if the body stream may be at the end, or
    /// if the stream will not yield any chunks, in all cases. For instance,
    /// a streaming body using `chunked` encoding is not able to tell if
    /// there are more chunks immediately.
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

    pub(crate) fn delayed_eof(&mut self, fut: DelayEofUntil) {
        self.delayed_eof = Some(DelayEof::NotEof(fut));
    }

    fn poll_eof(&mut self, cx: &mut task::Context) -> Poll<Option<Chunk>, ::Error> {
        match self.delayed_eof.take() {
            Some(DelayEof::NotEof(mut delay)) => {
                match self.poll_inner(cx) {
                    ok @ Ok(Async::Ready(Some(..))) |
                    ok @ Ok(Async::Pending) => {
                        self.delayed_eof = Some(DelayEof::NotEof(delay));
                        ok
                    },
                    Ok(Async::Ready(None)) => match delay.poll(cx) {
                        Ok(Async::Ready(never)) => match never {},
                        Ok(Async::Pending) => {
                            self.delayed_eof = Some(DelayEof::Eof(delay));
                            Ok(Async::Pending)
                        },
                        Err(_done) => {
                            Ok(Async::Ready(None))
                        },
                    },
                    Err(e) => Err(e),
                }
            },
            Some(DelayEof::Eof(mut delay)) => {
                match delay.poll(cx) {
                    Ok(Async::Ready(never)) => match never {},
                    Ok(Async::Pending) => {
                        self.delayed_eof = Some(DelayEof::Eof(delay));
                        Ok(Async::Pending)
                    },
                    Err(_done) => {
                        Ok(Async::Ready(None))
                    },
                }
            },
            None => self.poll_inner(cx),
        }
    }

    fn poll_inner(&mut self, cx: &mut task::Context) -> Poll<Option<Chunk>, ::Error> {
        match self.kind {
            Kind::Chan { ref mut rx, .. } => match rx.poll_next(cx).expect("mpsc cannot error") {
                Async::Ready(Some(Ok(chunk))) => Ok(Async::Ready(Some(chunk))),
                Async::Ready(Some(Err(err))) => Err(err),
                Async::Ready(None) => Ok(Async::Ready(None)),
                Async::Pending => Ok(Async::Pending),
            },
            Kind::Wrapped(ref mut s) => s.poll_next(cx),
            Kind::Once(ref mut val) => Ok(Async::Ready(val.take())),
            Kind::Empty => Ok(Async::Ready(None)),
        }
    }
}

impl Default for Body {
    #[inline]
    fn default() -> Body {
        Body::empty()
    }
}

impl Entity for Body {
    type Data = Chunk;
    type Error = ::Error;

    fn poll_data(&mut self, cx: &mut task::Context) -> Poll<Option<Self::Data>, Self::Error> {
        self.poll_eof(cx)
    }

    fn is_end_stream(&self) -> bool {
        match self.kind {
            Kind::Chan { .. } => false,
            Kind::Wrapped(..) => false,
            Kind::Once(ref val) => val.is_none(),
            Kind::Empty => true
        }
    }

    fn content_length(&self) -> Option<u64> {
        match self.kind {
            Kind::Chan { .. } => None,
            Kind::Wrapped(..) => None,
            Kind::Once(Some(ref val)) => Some(val.len() as u64),
            Kind::Once(None) => None,
            Kind::Empty => Some(0)
        }
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
    pub fn poll_ready(&mut self, cx: &mut task::Context) -> Poll<(), ()> {
        match self.close_rx.poll(cx) {
            Ok(Async::Ready(())) | Err(_) => return Err(()),
            Ok(Async::Pending) => (),
        }

        self.tx.poll_ready(cx).map_err(|_| ())
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
    use futures::{StreamExt};

    let body = Body::from("hello world");

    let total = ::futures::executor::block_on(body.into_stream().concat())
        .unwrap();
    assert_eq!(total.as_ref(), b"hello world");

}
