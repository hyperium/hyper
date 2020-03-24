use std::borrow::Cow;
#[cfg(feature = "stream")]
use std::error::Error as StdError;
use std::fmt;

use bytes::Bytes;
use futures_channel::{mpsc, oneshot};
use futures_core::Stream; // for mpsc::Receiver
#[cfg(feature = "stream")]
use futures_util::TryStreamExt;
use http::HeaderMap;
use http_body::{Body as HttpBody, SizeHint};

use crate::common::{task, watch, Future, Never, Pin, Poll};
use crate::proto::h2::ping;
use crate::proto::DecodedLength;
use crate::upgrade::OnUpgrade;

type BodySender = mpsc::Sender<Result<Bytes, crate::Error>>;

/// A stream of `Bytes`, used when receiving bodies.
///
/// A good default [`HttpBody`](crate::body::HttpBody) to use in many
/// applications.
#[must_use = "streams do nothing unless polled"]
pub struct Body {
    kind: Kind,
    /// Keep the extra bits in an `Option<Box<Extra>>`, so that
    /// Body stays small in the common case (no extras needed).
    extra: Option<Box<Extra>>,
}

enum Kind {
    Once(Option<Bytes>),
    Chan {
        content_length: DecodedLength,
        want_tx: watch::Sender,
        rx: mpsc::Receiver<Result<Bytes, crate::Error>>,
    },
    H2 {
        ping: ping::Recorder,
        content_length: DecodedLength,
        recv: h2::RecvStream,
    },
    // NOTE: This requires `Sync` because of how easy it is to use `await`
    // while a borrow of a `Request<Body>` exists.
    //
    // See https://github.com/rust-lang/rust/issues/57017
    #[cfg(feature = "stream")]
    Wrapped(
        Pin<Box<dyn Stream<Item = Result<Bytes, Box<dyn StdError + Send + Sync>>> + Send + Sync>>,
    ),
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
pub struct Sender {
    want_rx: watch::Receiver,
    tx: BodySender,
}

const WANT_PENDING: usize = 1;
const WANT_READY: usize = 2;

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
        Self::new_channel(DecodedLength::CHUNKED, /*wanter =*/ false)
    }

    pub(crate) fn new_channel(content_length: DecodedLength, wanter: bool) -> (Sender, Body) {
        let (tx, rx) = mpsc::channel(0);

        // If wanter is true, `Sender::poll_ready()` won't becoming ready
        // until the `Body` has been polled for data once.
        let want = if wanter { WANT_PENDING } else { WANT_READY };

        let (want_tx, want_rx) = watch::channel(want);

        let tx = Sender { want_rx, tx };
        let rx = Body::new(Kind::Chan {
            content_length,
            want_tx,
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
    /// let chunks: Vec<Result<_, std::io::Error>> = vec![
    ///     Ok("hello"),
    ///     Ok(" "),
    ///     Ok("world"),
    /// ];
    ///
    /// let stream = futures_util::stream::iter(chunks);
    ///
    /// let body = Body::wrap_stream(stream);
    /// ```
    ///
    /// # Optional
    ///
    /// This function requires enabling the `stream` feature in your
    /// `Cargo.toml`.
    #[cfg(feature = "stream")]
    pub fn wrap_stream<S, O, E>(stream: S) -> Body
    where
        S: Stream<Item = Result<O, E>> + Send + Sync + 'static,
        O: Into<Bytes> + 'static,
        E: Into<Box<dyn StdError + Send + Sync>> + 'static,
    {
        let mapped = stream.map_ok(Into::into).map_err(Into::into);
        Body::new(Kind::Wrapped(Box::pin(mapped)))
    }

    /// Converts this `Body` into a `Future` of a pending HTTP upgrade.
    ///
    /// See [the `upgrade` module](crate::upgrade) for more.
    pub fn on_upgrade(self) -> OnUpgrade {
        self.extra
            .map(|ex| ex.on_upgrade)
            .unwrap_or_else(OnUpgrade::none)
    }

    fn new(kind: Kind) -> Body {
        Body { kind, extra: None }
    }

    pub(crate) fn h2(
        recv: h2::RecvStream,
        content_length: DecodedLength,
        ping: ping::Recorder,
    ) -> Self {
        let body = Body::new(Kind::H2 {
            ping,
            content_length,
            recv,
        });

        body
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
        self.extra
            .as_mut()
            .and_then(|extra| extra.delayed_eof.take())
    }

    fn extra_mut(&mut self) -> &mut Extra {
        self.extra.get_or_insert_with(|| {
            Box::new(Extra {
                delayed_eof: None,
                on_upgrade: OnUpgrade::none(),
            })
        })
    }

    fn poll_eof(&mut self, cx: &mut task::Context<'_>) -> Poll<Option<crate::Result<Bytes>>> {
        match self.take_delayed_eof() {
            Some(DelayEof::NotEof(mut delay)) => match self.poll_inner(cx) {
                ok @ Poll::Ready(Some(Ok(..))) | ok @ Poll::Pending => {
                    self.extra_mut().delayed_eof = Some(DelayEof::NotEof(delay));
                    ok
                }
                Poll::Ready(None) => match Pin::new(&mut delay).poll(cx) {
                    Poll::Ready(Ok(never)) => match never {},
                    Poll::Pending => {
                        self.extra_mut().delayed_eof = Some(DelayEof::Eof(delay));
                        Poll::Pending
                    }
                    Poll::Ready(Err(_done)) => Poll::Ready(None),
                },
                Poll::Ready(Some(Err(e))) => Poll::Ready(Some(Err(e))),
            },
            Some(DelayEof::Eof(mut delay)) => match Pin::new(&mut delay).poll(cx) {
                Poll::Ready(Ok(never)) => match never {},
                Poll::Pending => {
                    self.extra_mut().delayed_eof = Some(DelayEof::Eof(delay));
                    Poll::Pending
                }
                Poll::Ready(Err(_done)) => Poll::Ready(None),
            },
            None => self.poll_inner(cx),
        }
    }

    fn poll_inner(&mut self, cx: &mut task::Context<'_>) -> Poll<Option<crate::Result<Bytes>>> {
        match self.kind {
            Kind::Once(ref mut val) => Poll::Ready(val.take().map(Ok)),
            Kind::Chan {
                content_length: ref mut len,
                ref mut rx,
                ref mut want_tx,
            } => {
                want_tx.send(WANT_READY);

                match ready!(Pin::new(rx).poll_next(cx)?) {
                    Some(chunk) => {
                        len.sub_if(chunk.len() as u64);
                        Poll::Ready(Some(Ok(chunk)))
                    }
                    None => Poll::Ready(None),
                }
            }
            Kind::H2 {
                ref ping,
                recv: ref mut h2,
                content_length: ref mut len,
            } => match ready!(h2.poll_data(cx)) {
                Some(Ok(bytes)) => {
                    let _ = h2.flow_control().release_capacity(bytes.len());
                    len.sub_if(bytes.len() as u64);
                    ping.record_data(bytes.len());
                    Poll::Ready(Some(Ok(bytes)))
                }
                Some(Err(e)) => Poll::Ready(Some(Err(crate::Error::new_body(e)))),
                None => Poll::Ready(None),
            },

            #[cfg(feature = "stream")]
            Kind::Wrapped(ref mut s) => match ready!(s.as_mut().poll_next(cx)) {
                Some(res) => Poll::Ready(Some(res.map_err(crate::Error::new_body))),
                None => Poll::Ready(None),
            },
        }
    }

    pub(super) fn take_full_data(&mut self) -> Option<Bytes> {
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
    type Data = Bytes;
    type Error = crate::Error;

    fn poll_data(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> Poll<Option<Result<Self::Data, Self::Error>>> {
        self.poll_eof(cx)
    }

    fn poll_trailers(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> Poll<Result<Option<HeaderMap>, Self::Error>> {
        match self.kind {
            Kind::H2 {
                recv: ref mut h2,
                ref ping,
                ..
            } => match ready!(h2.poll_trailers(cx)) {
                Ok(t) => {
                    ping.record_non_data();
                    Poll::Ready(Ok(t))
                }
                Err(e) => Poll::Ready(Err(crate::Error::new_h2(e))),
            },
            _ => Poll::Ready(Ok(None)),
        }
    }

    fn is_end_stream(&self) -> bool {
        match self.kind {
            Kind::Once(ref val) => val.is_none(),
            Kind::Chan { content_length, .. } => content_length == DecodedLength::ZERO,
            Kind::H2 { recv: ref h2, .. } => h2.is_end_stream(),
            #[cfg(feature = "stream")]
            Kind::Wrapped(..) => false,
        }
    }

    fn size_hint(&self) -> SizeHint {
        match self.kind {
            Kind::Once(Some(ref val)) => SizeHint::with_exact(val.len() as u64),
            Kind::Once(None) => SizeHint::with_exact(0),
            #[cfg(feature = "stream")]
            Kind::Wrapped(..) => SizeHint::default(),
            Kind::Chan { content_length, .. } | Kind::H2 { content_length, .. } => {
                let mut hint = SizeHint::default();

                if let Some(content_length) = content_length.into_opt() {
                    hint.set_exact(content_length);
                }

                hint
            }
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
        struct Full<'a>(&'a Bytes);

        let mut builder = f.debug_tuple("Body");
        match self.kind {
            Kind::Once(None) => builder.field(&Empty),
            Kind::Once(Some(ref chunk)) => builder.field(&Full(chunk)),
            _ => builder.field(&Streaming),
        };

        builder.finish()
    }
}

/// # Optional
///
/// This function requires enabling the `stream` feature in your
/// `Cargo.toml`.
#[cfg(feature = "stream")]
impl Stream for Body {
    type Item = crate::Result<Bytes>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Option<Self::Item>> {
        HttpBody::poll_data(self, cx)
    }
}

/// # Optional
///
/// This function requires enabling the `stream` feature in your
/// `Cargo.toml`.
#[cfg(feature = "stream")]
impl From<Box<dyn Stream<Item = Result<Bytes, Box<dyn StdError + Send + Sync>>> + Send + Sync>>
    for Body
{
    #[inline]
    fn from(
        stream: Box<
            dyn Stream<Item = Result<Bytes, Box<dyn StdError + Send + Sync>>> + Send + Sync,
        >,
    ) -> Body {
        Body::new(Kind::Wrapped(stream.into()))
    }
}

impl From<Bytes> for Body {
    #[inline]
    fn from(chunk: Bytes) -> Body {
        if chunk.is_empty() {
            Body::empty()
        } else {
            Body::new(Kind::Once(Some(chunk)))
        }
    }
}

impl From<Vec<u8>> for Body {
    #[inline]
    fn from(vec: Vec<u8>) -> Body {
        Body::from(Bytes::from(vec))
    }
}

impl From<&'static [u8]> for Body {
    #[inline]
    fn from(slice: &'static [u8]) -> Body {
        Body::from(Bytes::from(slice))
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
        Body::from(Bytes::from(s.into_bytes()))
    }
}

impl From<&'static str> for Body {
    #[inline]
    fn from(slice: &'static str) -> Body {
        Body::from(Bytes::from(slice.as_bytes()))
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
        // Check if the receiver end has tried polling for the body yet
        ready!(self.poll_want(cx)?);
        self.tx
            .poll_ready(cx)
            .map_err(|_| crate::Error::new_closed())
    }

    fn poll_want(&mut self, cx: &mut task::Context<'_>) -> Poll<crate::Result<()>> {
        match self.want_rx.load(cx) {
            WANT_READY => Poll::Ready(Ok(())),
            WANT_PENDING => Poll::Pending,
            watch::CLOSED => Poll::Ready(Err(crate::Error::new_closed())),
            unexpected => unreachable!("want_rx value: {}", unexpected),
        }
    }

    async fn ready(&mut self) -> crate::Result<()> {
        futures_util::future::poll_fn(|cx| self.poll_ready(cx)).await
    }

    /// Send data on this channel when it is ready.
    pub async fn send_data(&mut self, chunk: Bytes) -> crate::Result<()> {
        self.ready().await?;
        self.tx
            .try_send(Ok(chunk))
            .map_err(|_| crate::Error::new_closed())
    }

    /// Try to send data on this channel.
    ///
    /// # Errors
    ///
    /// Returns `Err(Bytes)` if the channel could not (currently) accept
    /// another `Bytes`.
    ///
    /// # Note
    ///
    /// This is mostly useful for when trying to send from some other thread
    /// that doesn't have an async context. If in an async context, prefer
    /// `send_data()` instead.
    pub fn try_send_data(&mut self, chunk: Bytes) -> Result<(), Bytes> {
        self.tx
            .try_send(Ok(chunk))
            .map_err(|err| err.into_inner().expect("just sent Ok"))
    }

    /// Aborts the body in an abnormal fashion.
    pub fn abort(self) {
        let _ = self
            .tx
            // clone so the send works even if buffer is full
            .clone()
            .try_send(Err(crate::Error::new_body_write_aborted()));
    }

    pub(crate) fn send_error(&mut self, err: crate::Error) {
        let _ = self.tx.try_send(Err(err));
    }
}

impl fmt::Debug for Sender {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        #[derive(Debug)]
        struct Open;
        #[derive(Debug)]
        struct Closed;

        let mut builder = f.debug_tuple("Sender");
        match self.want_rx.peek() {
            watch::CLOSED => builder.field(&Closed),
            _ => builder.field(&Open),
        };

        builder.finish()
    }
}

#[cfg(test)]
mod tests {
    use std::mem;
    use std::task::Poll;

    use super::{Body, DecodedLength, HttpBody, Sender, SizeHint};

    #[test]
    fn test_size_of() {
        // These are mostly to help catch *accidentally* increasing
        // the size by too much.

        let body_size = mem::size_of::<Body>();
        let body_expected_size = mem::size_of::<u64>() * 6;
        assert!(
            body_size <= body_expected_size,
            "Body size = {} <= {}",
            body_size,
            body_expected_size,
        );

        assert_eq!(body_size, mem::size_of::<Option<Body>>(), "Option<Body>");

        assert_eq!(
            mem::size_of::<Sender>(),
            mem::size_of::<usize>() * 4,
            "Sender"
        );

        assert_eq!(
            mem::size_of::<Sender>(),
            mem::size_of::<Option<Sender>>(),
            "Option<Sender>"
        );
    }

    #[test]
    fn size_hint() {
        fn eq(body: Body, b: SizeHint, note: &str) {
            let a = body.size_hint();
            assert_eq!(a.lower(), b.lower(), "lower for {:?}", note);
            assert_eq!(a.upper(), b.upper(), "upper for {:?}", note);
        }

        eq(Body::from("Hello"), SizeHint::with_exact(5), "from str");

        eq(Body::empty(), SizeHint::with_exact(0), "empty");

        eq(Body::channel().1, SizeHint::new(), "channel");

        eq(
            Body::new_channel(DecodedLength::new(4), /*wanter =*/ false).1,
            SizeHint::with_exact(4),
            "channel with length",
        );
    }

    #[tokio::test]
    async fn channel_abort() {
        let (tx, mut rx) = Body::channel();

        tx.abort();

        let err = rx.data().await.unwrap().unwrap_err();
        assert!(err.is_body_write_aborted(), "{:?}", err);
    }

    #[tokio::test]
    async fn channel_abort_when_buffer_is_full() {
        let (mut tx, mut rx) = Body::channel();

        tx.try_send_data("chunk 1".into()).expect("send 1");
        // buffer is full, but can still send abort
        tx.abort();

        let chunk1 = rx.data().await.expect("item 1").expect("chunk 1");
        assert_eq!(chunk1, "chunk 1");

        let err = rx.data().await.unwrap().unwrap_err();
        assert!(err.is_body_write_aborted(), "{:?}", err);
    }

    #[test]
    fn channel_buffers_one() {
        let (mut tx, _rx) = Body::channel();

        tx.try_send_data("chunk 1".into()).expect("send 1");

        // buffer is now full
        let chunk2 = tx.try_send_data("chunk 2".into()).expect_err("send 2");
        assert_eq!(chunk2, "chunk 2");
    }

    #[tokio::test]
    async fn channel_empty() {
        let (_, mut rx) = Body::channel();

        assert!(rx.data().await.is_none());
    }

    #[test]
    fn channel_ready() {
        let (mut tx, _rx) = Body::new_channel(DecodedLength::CHUNKED, /*wanter = */ false);

        let mut tx_ready = tokio_test::task::spawn(tx.ready());

        assert!(tx_ready.poll().is_ready(), "tx is ready immediately");
    }

    #[test]
    fn channel_wanter() {
        let (mut tx, mut rx) = Body::new_channel(DecodedLength::CHUNKED, /*wanter = */ true);

        let mut tx_ready = tokio_test::task::spawn(tx.ready());
        let mut rx_data = tokio_test::task::spawn(rx.data());

        assert!(
            tx_ready.poll().is_pending(),
            "tx isn't ready before rx has been polled"
        );

        assert!(rx_data.poll().is_pending(), "poll rx.data");
        assert!(tx_ready.is_woken(), "rx poll wakes tx");

        assert!(
            tx_ready.poll().is_ready(),
            "tx is ready after rx has been polled"
        );
    }

    #[test]
    fn channel_notices_closure() {
        let (mut tx, rx) = Body::new_channel(DecodedLength::CHUNKED, /*wanter = */ true);

        let mut tx_ready = tokio_test::task::spawn(tx.ready());

        assert!(
            tx_ready.poll().is_pending(),
            "tx isn't ready before rx has been polled"
        );

        drop(rx);
        assert!(tx_ready.is_woken(), "dropping rx wakes tx");

        match tx_ready.poll() {
            Poll::Ready(Err(ref e)) if e.is_closed() => (),
            unexpected => panic!("tx poll ready unexpected: {:?}", unexpected),
        }
    }
}
