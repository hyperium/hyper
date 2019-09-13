use std::borrow::Cow;
#[cfg(feature = "unstable-stream")]
use std::error::Error as StdError;
use std::fmt;

use bytes::Bytes;
use futures_core::Stream; // for mpsc::Receiver
use futures_channel::{mpsc, oneshot};
#[cfg(feature = "unstable-stream")]
use futures_util::TryStreamExt;
use http_body::{SizeHint, Body as HttpBody};
use http::HeaderMap;

use crate::common::{Future, Never, Pin, Poll, task};
use super::Chunk;
use crate::upgrade::OnUpgrade;

type BodySender = mpsc::Sender<Result<Chunk, crate::Error>>;

/// A stream of `Chunk`s, used when receiving bodies.
///
/// A good default `Payload` to use in many applications.
#[must_use = "streams do nothing unless polled"]
pub struct Body {
    kind: Kind,
    /// Keep the extra bits in an `Option<Box<Extra>>`, so that
    /// Body stays small in the common case (no extras needed).
    extra: Option<Box<Extra>>,
}

enum Kind {
    Once(Option<Chunk>),
    Chan {
        content_length: Option<u64>,
        abort_rx: oneshot::Receiver<()>,
        rx: mpsc::Receiver<Result<Chunk, crate::Error>>,
    },
    H2 {
        content_length: Option<u64>,
        recv: h2::RecvStream,
    },
    // NOTE: This requires `Sync` because of how easy it is to use `await`
    // while a borrow of a `Request<Body>` exists.
    //
    // See https://github.com/rust-lang/rust/issues/57017
    #[cfg(feature = "unstable-stream")]
    Wrapped(Pin<Box<dyn Stream<Item = Result<Chunk, Box<dyn StdError + Send + Sync>>> + Send + Sync>>),
}

struct Extra {
    /// Allow the client to pass a future to delay the `Body` from returning
    /// EOF. This allows the `Client` to try to put the idle connection
    /// back into the pool before the body is "finished".
    ///
    /// The reason for this is so that creating a new request after finishing
    /// streaming the body of a response could sometimes result in creating
    /// a brand new connection, since the pool didn't know about the idle
    /// connection yet.
    delayed_eof: Option<DelayEof>,
    on_upgrade: OnUpgrade,
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
    /// # use hyper::Body;
    /// # fn main() {
    /// let chunks: Vec<Result<_, ::std::io::Error>> = vec![
    ///     Ok("hello"),
    ///     Ok(" "),
    ///     Ok("world"),
    /// ];
    ///
    /// let stream = futures_util::stream::iter(chunks);
    ///
    /// let body = Body::wrap_stream(stream);
    /// # }
    /// ```
    ///
    /// # Unstable
    ///
    /// This function requires enabling the `unstable-stream` feature in your
    /// `Cargo.toml`.
    #[cfg(feature = "unstable-stream")]
    pub fn wrap_stream<S, O, E>(stream: S) -> Body
    where
        S: Stream<Item = Result<O, E>> + Send + Sync + 'static,
        O: Into<Chunk> + 'static,
        E: Into<Box<dyn StdError + Send + Sync>> + 'static,
    {
        let mapped = stream.map_ok(Into::into).map_err(Into::into);
        Body::new(Kind::Wrapped(Box::pin(mapped)))
    }

    /// dox
    pub async fn next(&mut self) -> Option<crate::Result<Chunk>> {
        futures_util::future::poll_fn(|cx| self.poll_eof(cx)).await
    }

    /// Converts this `Body` into a `Future` of a pending HTTP upgrade.
    ///
    /// See [the `upgrade` module](::upgrade) for more.
    pub fn on_upgrade(self) -> OnUpgrade {
        self
            .extra
            .map(|ex| ex.on_upgrade)
            .unwrap_or_else(OnUpgrade::none)
    }

    fn new(kind: Kind) -> Body {
        Body {
            kind: kind,
            extra: None,
        }
    }

    pub(crate) fn h2(recv: h2::RecvStream, content_length: Option<u64>) -> Self {
        Body::new(Kind::H2 {
            content_length,
            recv,
        })
    }

    pub(crate) fn set_on_upgrade(&mut self, upgrade: OnUpgrade) {
        debug_assert!(!upgrade.is_none(), "set_on_upgrade with empty upgrade");
        let extra = self.extra_mut();
        debug_assert!(extra.on_upgrade.is_none(), "set_on_upgrade twice");
        extra.on_upgrade = upgrade;
    }

    pub(crate) fn delayed_eof(&mut self, fut: DelayEofUntil) {
        self.extra_mut().delayed_eof = Some(DelayEof::NotEof(fut));
    }

    fn take_delayed_eof(&mut self) -> Option<DelayEof> {
        self
            .extra
            .as_mut()
            .and_then(|extra| extra.delayed_eof.take())
    }

    fn extra_mut(&mut self) -> &mut Extra {
        self
            .extra
            .get_or_insert_with(|| Box::new(Extra {
                delayed_eof: None,
                on_upgrade: OnUpgrade::none(),
            }))
    }

    fn poll_eof(&mut self, cx: &mut task::Context<'_>) -> Poll<Option<crate::Result<Chunk>>> {
        match self.take_delayed_eof() {
            Some(DelayEof::NotEof(mut delay)) => {
                match self.poll_inner(cx) {
                    ok @ Poll::Ready(Some(Ok(..))) |
                    ok @ Poll::Pending => {
                        self.extra_mut().delayed_eof = Some(DelayEof::NotEof(delay));
                        ok
                    },
                    Poll::Ready(None) => match Pin::new(&mut delay).poll(cx) {
                        Poll::Ready(Ok(never)) => match never {},
                        Poll::Pending => {
                            self.extra_mut().delayed_eof = Some(DelayEof::Eof(delay));
                            Poll::Pending
                        },
                        Poll::Ready(Err(_done)) => {
                            Poll::Ready(None)
                        },
                    },
                    Poll::Ready(Some(Err(e))) => Poll::Ready(Some(Err(e))),
                }
            },
            Some(DelayEof::Eof(mut delay)) => {
                match Pin::new(&mut delay).poll(cx) {
                    Poll::Ready(Ok(never)) => match never {},
                    Poll::Pending => {
                        self.extra_mut().delayed_eof = Some(DelayEof::Eof(delay));
                        Poll::Pending
                    },
                    Poll::Ready(Err(_done)) => {
                        Poll::Ready(None)
                    },
                }
            },
            None => self.poll_inner(cx),
        }
    }

    fn poll_inner(&mut self, cx: &mut task::Context<'_>) -> Poll<Option<crate::Result<Chunk>>> {
        match self.kind {
            Kind::Once(ref mut val) => Poll::Ready(val.take().map(Ok)),
            Kind::Chan {
                content_length: ref mut len,
                ref mut rx,
                ref mut abort_rx,
            } => {
                if let Poll::Ready(Ok(())) = Pin::new(abort_rx).poll(cx) {
                    return Poll::Ready(Some(Err(crate::Error::new_body_write_aborted())));
                }

                match ready!(Pin::new(rx).poll_next(cx)?) {
                    Some(chunk) => {
                        if let Some(ref mut len) = *len {
                            debug_assert!(*len >= chunk.len() as u64);
                            *len = *len - chunk.len() as u64;
                        }
                        Poll::Ready(Some(Ok(chunk)))
                    }
                    None => Poll::Ready(None),
                }
            },
            Kind::H2 {
                recv: ref mut h2, ..
            } => match ready!(h2.poll_data(cx)) {
                Some(Ok(bytes)) => {
                    let _ = h2.release_capacity().release_capacity(bytes.len());
                    Poll::Ready(Some(Ok(Chunk::from(bytes))))
                },
                Some(Err(e)) => Poll::Ready(Some(Err(crate::Error::new_body(e)))),
                None => Poll::Ready(None),
            },

            #[cfg(feature = "unstable-stream")]
            Kind::Wrapped(ref mut s) => {
                match ready!(s.as_mut().poll_next(cx)) {
                    Some(res) => Poll::Ready(Some(res.map_err(crate::Error::new_body))),
                    None => Poll::Ready(None),
                }
            }
        }
    }

    pub(super) fn take_full_data(&mut self) -> Option<Chunk> {
        if let Kind::Once(ref mut chunk) = self.kind {
            chunk.take()
        } else {
            None
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

impl HttpBody for Body {
    type Data = Chunk;
    type Error = crate::Error;

    fn poll_data(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Option<Result<Self::Data, Self::Error>>> {
        self.poll_eof(cx)
    }

    fn poll_trailers(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Result<Option<HeaderMap>, Self::Error>> {
        match self.kind {
            Kind::H2 { recv: ref mut h2, .. } => match ready!(h2.poll_trailers(cx)) {
                Ok(t) => Poll::Ready(Ok(t)),
                Err(e) => Poll::Ready(Err(crate::Error::new_h2(e))),
            },
            _ => Poll::Ready(Ok(None)),
        }
    }

    fn is_end_stream(&self) -> bool {
        match self.kind {
            Kind::Once(ref val) => val.is_none(),
            Kind::Chan { content_length, .. } => content_length == Some(0),
            Kind::H2 { recv: ref h2, .. } => h2.is_end_stream(),
            #[cfg(feature = "unstable-stream")]
            Kind::Wrapped(..) => false,
        }
    }

    fn size_hint(&self) -> SizeHint {
        match self.kind {
            Kind::Once(Some(ref val)) => {
                let mut hint = SizeHint::default();
                hint.set_exact(val.len() as u64);
                hint
            },
            Kind::Once(None) => {
                SizeHint::default()
            },
            #[cfg(feature = "unstable-stream")]
            Kind::Wrapped(..) => SizeHint::default(),
            Kind::Chan { content_length, .. } | Kind::H2 { content_length, .. } => {
                let mut hint = SizeHint::default();

                if let Some(content_length) = content_length {
                    hint.set_exact(content_length as u64);
                }

                hint
            },
        }
    }
}

impl fmt::Debug for Body {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        #[derive(Debug)]
        struct Streaming;
        #[derive(Debug)]
        struct Empty;
        #[derive(Debug)]
        struct Full<'a>(&'a Chunk);

        let mut builder = f.debug_tuple("Body");
        match self.kind {
            Kind::Once(None) => builder.field(&Empty),
            Kind::Once(Some(ref chunk)) => builder.field(&Full(chunk)),
            _ => builder.field(&Streaming),
        };

        builder.finish()
    }
}

/// # Unstable
///
/// This function requires enabling the `unstable-stream` feature in your
/// `Cargo.toml`.
#[cfg(feature = "unstable-stream")]
impl Stream for Body {
    type Item = crate::Result<Chunk>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Option<Self::Item>> {
        HttpBody::poll_data(self, cx)
    }
}


/// # Unstable
///
/// This function requires enabling the `unstable-stream` feature in your
/// `Cargo.toml`.
#[cfg(feature = "unstable-stream")]
impl
    From<Box<dyn Stream<Item = Result<Chunk, Box<dyn StdError + Send + Sync>>> + Send + Sync>>
    for Body
{
    #[inline]
    fn from(
        stream: Box<
            dyn Stream<Item = Result<Chunk, Box<dyn StdError + Send + Sync>>> + Send + Sync,
        >,
    ) -> Body {
        Body::new(Kind::Wrapped(stream.into()))
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
    fn from(bytes: Bytes) -> Body {
        Body::from(Chunk::from(bytes))
    }
}

impl From<Vec<u8>> for Body {
    #[inline]
    fn from(vec: Vec<u8>) -> Body {
        Body::from(Chunk::from(vec))
    }
}

impl From<&'static [u8]> for Body {
    #[inline]
    fn from(slice: &'static [u8]) -> Body {
        Body::from(Chunk::from(slice))
    }
}

impl From<Cow<'static, [u8]>> for Body {
    #[inline]
    fn from(cow: Cow<'static, [u8]>) -> Body {
        match cow {
            Cow::Borrowed(b) => Body::from(b),
            Cow::Owned(o) => Body::from(o),
        }
    }
}

impl From<String> for Body {
    #[inline]
    fn from(s: String) -> Body {
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
            Cow::Owned(o) => Body::from(o),
        }
    }
}

impl Sender {
    /// Check to see if this `Sender` can send more data.
    pub fn poll_ready(&mut self, cx: &mut task::Context<'_>) -> Poll<crate::Result<()>> {
        match self.abort_tx.poll_cancel(cx) {
            Poll::Ready(()) => return Poll::Ready(Err(crate::Error::new_closed())),
            Poll::Pending => (), // fallthrough
        }

        self.tx.poll_ready(cx).map_err(|_| crate::Error::new_closed())
    }

    /// Send data on this channel when it is ready.
    pub async fn send_data(&mut self, chunk: Chunk) -> crate::Result<()> {
        futures_util::future::poll_fn(|cx| self.poll_ready(cx)).await?;
        self.tx.try_send(Ok(chunk)).map_err(|_| crate::Error::new_closed())
    }

    /// Try to send data on this channel.
    ///
    /// # Errors
    ///
    /// Returns `Err(Chunk)` if the channel could not (currently) accept
    /// another `Chunk`.
    ///
    /// # Note
    ///
    /// This is mostly useful for when trying to send from some other thread
    /// that doesn't have an async context. If in an async context, prefer
    /// [`send_data`][] instead.
    pub fn try_send_data(&mut self, chunk: Chunk) -> Result<(), Chunk> {
        self.tx
            .try_send(Ok(chunk))
            .map_err(|err| err.into_inner().expect("just sent Ok"))
    }

    /// Aborts the body in an abnormal fashion.
    pub fn abort(self) {
        let _ = self.abort_tx.send(());
    }

    pub(crate) fn send_error(&mut self, err: crate::Error) {
        let _ = self.tx.try_send(Err(err));
    }
}

/*
impl Sink for Sender {
    type SinkItem = Chunk;
    type SinkError = crate::Error;

    fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
        Poll::Ready(Ok(()))
    }

    fn start_send(&mut self, msg: Chunk) -> StartSend<Self::SinkItem, Self::SinkError> {
        match self.poll_ready()? {
            Async::Ready(_) => {
                self.send_data(msg).map_err(|_| crate::Error::new_closed())?;
                Ok(AsyncSink::Ready)
            }
            Async::NotReady => Ok(AsyncSink::NotReady(msg)),
        }
    }
}
*/
