use std::borrow::Cow;
use std::fmt;

use bytes::Bytes;
use futures::{Async, Future, Poll, Stream};
use futures::sync::{mpsc, oneshot};
use h2;
use http::HeaderMap;

use common::Never;
use super::{Chunk, Payload};
use super::internal::{FullDataArg, FullDataRet};

type BodySender = mpsc::Sender<Result<Chunk, ::Error>>;

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
    Once(Option<Chunk>),
    Chan {
        content_length: Option<u64>,
        abort_rx: oneshot::Receiver<()>,
        rx: mpsc::Receiver<Result<Chunk, ::Error>>,
    },
    H2(h2::RecvStream),
    Wrapped(Box<Stream<Item=Chunk, Error=Box<::std::error::Error + Send + Sync>> + Send>),
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
    abort_tx: oneshot::Sender<()>,
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
        Body::new(Kind::Once(None))
    }

    /// Create a `Body` stream with an associated sender half.
    ///
    /// Useful when wanting to stream chunks from another thread.
    #[inline]
    pub fn channel() -> (Sender, Body) {
        Self::new_channel(None)
    }

    #[inline]
    pub(crate) fn new_channel(content_length: Option<u64>) -> (Sender, Body) {
        let (tx, rx) = mpsc::channel(0);
        let (abort_tx, abort_rx) = oneshot::channel();

        let tx = Sender {
            abort_tx: abort_tx,
            tx: tx,
        };
        let rx = Body::new(Kind::Chan {
            content_length,
            abort_rx,
            rx,
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
            Kind::Once(ref mut val) => Ok(Async::Ready(val.take())),
            Kind::Chan { content_length: ref mut len, ref mut rx, ref mut abort_rx } => {
                if let Ok(Async::Ready(())) = abort_rx.poll() {
                    return Err(::Error::new_body_write("body write aborted"));
                }

                match rx.poll().expect("mpsc cannot error") {
                    Async::Ready(Some(Ok(chunk))) => {
                        if let Some(ref mut len) = *len {
                            debug_assert!(*len >= chunk.len() as u64);
                            *len = *len - chunk.len() as u64;
                        }
                        Ok(Async::Ready(Some(chunk)))
                    }
                    Async::Ready(Some(Err(err))) => Err(err),
                    Async::Ready(None) => Ok(Async::Ready(None)),
                    Async::NotReady => Ok(Async::NotReady),
                }
            },
            Kind::H2(ref mut h2) => {
                h2.poll()
                    .map(|async| {
                        async.map(|opt| {
                            opt.map(|bytes| {
                                let _ = h2.release_capacity().release_capacity(bytes.len());
                                Chunk::from(bytes)
                            })
                        })
                    })
                    .map_err(::Error::new_body)
            },
            Kind::Wrapped(ref mut s) => s.poll().map_err(::Error::new_body),
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
            Kind::Once(ref val) => val.is_none(),
            Kind::Chan { content_length: len, .. } => len == Some(0),
            Kind::H2(ref h2) => h2.is_end_stream(),
            Kind::Wrapped(..) => false,
        }
    }

    fn content_length(&self) -> Option<u64> {
        match self.kind {
            Kind::Once(Some(ref val)) => Some(val.len() as u64),
            Kind::Once(None) => Some(0),
            Kind::Chan { content_length: len, .. } => len,
            Kind::H2(..) => None,
            Kind::Wrapped(..) => None,
        }
    }

    // We can improve the performance of `Body` when we know it is a Once kind.
    #[doc(hidden)]
    fn __hyper_full_data(&mut self, _: FullDataArg) -> FullDataRet<Self::Data> {
        match self.kind {
            Kind::Once(ref mut val) => FullDataRet(val.take()),
            _ => FullDataRet(None),
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
        match self.abort_tx.poll_cancel() {
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

    /// Aborts the body in an abnormal fashion.
    pub fn abort(self) {
        let _ = self.abort_tx.send(());
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

impl
    From<Box<Stream<Item = Chunk, Error = Box<::std::error::Error + Send + Sync>> + Send + 'static>>
    for Body
{
    #[inline]
    fn from(
        stream: Box<
            Stream<Item = Chunk, Error = Box<::std::error::Error + Send + Sync>> + Send + 'static,
        >,
    ) -> Body {
        Body::new(Kind::Wrapped(stream))
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

#[test]
fn test_body_stream_concat() {
    let body = Body::from("hello world");

    let total = body
        .concat2()
        .wait()
        .unwrap();
    assert_eq!(total.as_ref(), b"hello world");
}

