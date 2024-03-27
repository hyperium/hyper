use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::Bytes;
use futures_channel::{mpsc, oneshot};
use futures_util::ready;
use futures_util::{stream::FusedStream, Stream}; // for mpsc::Receiver
use http::HeaderMap;
use http_body::{Frame, SizeHint};

use crate::body::DecodedLength;
use crate::common::watch;

type BodySender = mpsc::Sender<Result<Bytes, crate::Error>>;
type TrailersSender = oneshot::Sender<HeaderMap>;

const WANT_PENDING: usize = 1;
const WANT_READY: usize = 2;

pub(super) struct ChanBody {
    content_length: DecodedLength,
    want_tx: watch::Sender,
    data_rx: mpsc::Receiver<Result<Bytes, crate::Error>>,
    trailers_rx: oneshot::Receiver<HeaderMap>,
}

impl ChanBody {
    pub(super) fn new(content_length: DecodedLength, wanter: bool) -> (Sender, Self) {
        let (data_tx, data_rx) = mpsc::channel(0);
        let (trailers_tx, trailers_rx) = oneshot::channel();

        // If wanter is true, `Sender::poll_ready()` won't becoming ready
        // until the `Body` has been polled for data once.
        let want = if wanter { WANT_PENDING } else { WANT_READY };

        let (want_tx, want_rx) = watch::channel(want);

        let tx = Sender {
            want_rx,
            data_tx,
            trailers_tx: Some(trailers_tx),
        };
        let rx = Self {
            content_length,
            want_tx,
            data_rx,
            trailers_rx,
        };

        (tx, rx)
    }

    pub(super) fn poll_frame(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, crate::Error>>> {
        let Self {
            content_length: ref mut len,
            ref mut data_rx,
            ref mut want_tx,
            ref mut trailers_rx,
        } = self;

        want_tx.send(WANT_READY);

        if !data_rx.is_terminated() {
            if let Some(chunk) = ready!(Pin::new(data_rx).poll_next(cx)?) {
                len.sub_if(chunk.len() as u64);
                return Poll::Ready(Some(Ok(Frame::data(chunk))));
            }
        }

        // check trailers after data is terminated
        match ready!(Pin::new(trailers_rx).poll(cx)) {
            Ok(t) => Poll::Ready(Some(Ok(Frame::trailers(t)))),
            Err(_) => Poll::Ready(None),
        }
    }

    pub(super) fn is_end_stream(&self) -> bool {
        self.content_length == DecodedLength::ZERO
    }

    pub(super) fn size_hint(&self) -> SizeHint {
        self.content_length
            .into_opt()
            .map(SizeHint::with_exact)
            .unwrap_or_default()
    }
}

/// A sender half created through [`Body::channel()`].
///
/// Useful when wanting to stream chunks from another thread.
///
/// ## Body Closing
///
/// Note that the request body will always be closed normally when the sender is dropped (meaning
/// that the empty terminating chunk will be sent to the remote). If you desire to close the
/// connection with an incomplete response (e.g. in the case of an error during asynchronous
/// processing), call the [`Sender::abort()`] method to abort the body in an abnormal fashion.
///
/// [`Body::channel()`]: struct.Body.html#method.channel
/// [`Sender::abort()`]: struct.Sender.html#method.abort
#[must_use = "Sender does nothing unless sent on"]
pub(crate) struct Sender {
    want_rx: watch::Receiver,
    data_tx: BodySender,
    trailers_tx: Option<TrailersSender>,
}

impl Sender {
    /// Check to see if this `Sender` can send more data.
    pub(crate) fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<crate::Result<()>> {
        // Check if the receiver end has tried polling for the body yet
        ready!(self.poll_want(cx)?);
        self.data_tx
            .poll_ready(cx)
            .map_err(|_| crate::Error::new_closed())
    }

    pub(crate) fn poll_want(&mut self, cx: &mut Context<'_>) -> Poll<crate::Result<()>> {
        match self.want_rx.load(cx) {
            WANT_READY => Poll::Ready(Ok(())),
            WANT_PENDING => Poll::Pending,
            watch::CLOSED => Poll::Ready(Err(crate::Error::new_closed())),
            unexpected => unreachable!("want_rx value: {}", unexpected),
        }
    }

    /// Send trailers on trailers channel.
    #[allow(unused)]
    pub(crate) async fn send_trailers(&mut self, trailers: HeaderMap) -> crate::Result<()> {
        let tx = match self.trailers_tx.take() {
            Some(tx) => tx,
            None => return Err(crate::Error::new_closed()),
        };
        tx.send(trailers).map_err(|_| crate::Error::new_closed())
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
    pub(crate) fn try_send_data(&mut self, chunk: Bytes) -> Result<(), Bytes> {
        self.data_tx
            .try_send(Ok(chunk))
            .map_err(|err| err.into_inner().expect("just sent Ok"))
    }

    pub(crate) fn send_error(&mut self, err: crate::Error) {
        let _ = self
            .data_tx
            // clone so the send works even if buffer is full
            .clone()
            .try_send(Err(err));
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
