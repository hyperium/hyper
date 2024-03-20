#[cfg(all(feature = "http1", any(feature = "client", feature = "server")))]
mod channel;
#[cfg(all(feature = "http2", any(feature = "client", feature = "server")))]
mod h2;

use std::fmt;
use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::Bytes;
use http_body::{Body, Frame, SizeHint};

#[cfg(all(feature = "http1", any(feature = "client", feature = "server")))]
use self::channel::ChanBody;
#[cfg(all(feature = "http1", any(feature = "client", feature = "server")))]
pub(crate) use self::channel::Sender;

#[cfg(all(feature = "http2", any(feature = "client", feature = "server")))]
use self::h2::H2Body;

#[cfg(all(
    any(feature = "http1", feature = "http2"),
    any(feature = "client", feature = "server")
))]
use super::DecodedLength;
#[cfg(all(feature = "http2", any(feature = "client", feature = "server")))]
use crate::proto::h2::ping;

/// A stream of `Bytes`, used when receiving bodies from the network.
///
/// Note that Users should not instantiate this struct directly. When working with the hyper client,
/// `Incoming` is returned to you in responses. Similarly, when operating with the hyper server,
/// it is provided within requests.
///
/// # Examples
///
/// ```rust,ignore
/// async fn echo(
///    req: Request<hyper::body::Incoming>,
/// ) -> Result<Response<BoxBody<Bytes, hyper::Error>>, hyper::Error> {
///    //Here, you can process `Incoming`
/// }
/// ```
#[must_use = "streams do nothing unless polled"]
pub struct Incoming {
    kind: Kind,
}

enum Kind {
    Empty,
    #[cfg(all(feature = "http1", any(feature = "client", feature = "server")))]
    Chan(ChanBody),
    #[cfg(all(feature = "http2", any(feature = "client", feature = "server")))]
    H2(H2Body),
    #[cfg(feature = "ffi")]
    Ffi(crate::ffi::UserBody),
}

impl Incoming {
    fn new(kind: Kind) -> Incoming {
        Incoming { kind }
    }

    #[allow(dead_code)]
    pub(crate) fn empty() -> Incoming {
        Incoming::new(Kind::Empty)
    }
}

#[cfg(any(feature = "client", feature = "server"))]
impl Incoming {
    #[cfg(feature = "http1")]
    pub(crate) fn channel(content_length: DecodedLength, wanter: bool) -> (Sender, Incoming) {
        let (tx, chan) = ChanBody::new(content_length, wanter);
        (tx, Incoming::new(Kind::Chan(chan)))
    }

    #[cfg(feature = "http2")]
    pub(crate) fn h2(
        recv: ::h2::RecvStream,
        content_length: DecodedLength,
        ping: ping::Recorder,
    ) -> Self {
        Incoming::new(Kind::H2(H2Body::new(recv, content_length, ping)))
    }
}

#[cfg(feature = "ffi")]
impl Incoming {
    pub(crate) fn ffi() -> Incoming {
        Incoming::new(Kind::Ffi(crate::ffi::UserBody::new()))
    }

    pub(crate) fn as_ffi_mut(&mut self) -> &mut crate::ffi::UserBody {
        match self.kind {
            Kind::Ffi(ref mut body) => return body,
            _ => {
                self.kind = Kind::Ffi(crate::ffi::UserBody::new());
            }
        }

        match self.kind {
            Kind::Ffi(ref mut body) => body,
            _ => unreachable!(),
        }
    }
}

impl Body for Incoming {
    type Data = Bytes;
    type Error = crate::Error;

    fn poll_frame(
        #[cfg_attr(
            not(all(
                any(feature = "http1", feature = "http2"),
                any(feature = "client", feature = "server")
            )),
            allow(unused_mut)
        )]
        mut self: Pin<&mut Self>,
        #[cfg_attr(
            not(all(
                any(feature = "http1", feature = "http2"),
                any(feature = "client", feature = "server")
            )),
            allow(unused_variables)
        )]
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        match self.kind {
            Kind::Empty => Poll::Ready(None),
            #[cfg(all(feature = "http1", any(feature = "client", feature = "server")))]
            Kind::Chan(ref mut body) => body.poll_frame(cx),
            #[cfg(all(feature = "http2", any(feature = "client", feature = "server")))]
            Kind::H2(ref mut body) => body.poll_frame(cx),
            #[cfg(feature = "ffi")]
            Kind::Ffi(ref mut body) => body.poll_data(cx),
        }
    }

    fn is_end_stream(&self) -> bool {
        match self.kind {
            Kind::Empty => true,
            #[cfg(all(feature = "http1", any(feature = "client", feature = "server")))]
            Kind::Chan(ref body) => body.is_end_stream(),
            #[cfg(all(feature = "http2", any(feature = "client", feature = "server")))]
            Kind::H2(ref body) => body.is_end_stream(),
            #[cfg(feature = "ffi")]
            Kind::Ffi(..) => false,
        }
    }

    fn size_hint(&self) -> SizeHint {
        match self.kind {
            Kind::Empty => SizeHint::with_exact(0),
            #[cfg(all(feature = "http1", any(feature = "client", feature = "server")))]
            Kind::Chan(ref body) => body.size_hint(),
            #[cfg(all(feature = "http2", any(feature = "client", feature = "server")))]
            Kind::H2(ref body) => body.size_hint(),
            #[cfg(feature = "ffi")]
            Kind::Ffi(..) => SizeHint::default(),
        }
    }
}

#[cfg(all(
    any(feature = "http1", feature = "http2"),
    any(feature = "client", feature = "server")
))]
fn opt_len(decoded_length: DecodedLength) -> SizeHint {
    if let Some(content_length) = decoded_length.into_opt() {
        SizeHint::with_exact(content_length)
    } else {
        SizeHint::default()
    }
}

impl fmt::Debug for Incoming {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        #[derive(Debug)]
        struct Streaming;
        #[derive(Debug)]
        struct Empty;

        let mut builder = f.debug_tuple("Body");
        match self.kind {
            Kind::Empty => builder.field(&Empty),
            #[cfg(any(
                all(
                    any(feature = "http1", feature = "http2"),
                    any(feature = "client", feature = "server")
                ),
                feature = "ffi"
            ))]
            _ => builder.field(&Streaming),
        };

        builder.finish()
    }
}

#[cfg(test)]
mod tests {
    use std::mem;
    use std::task::Poll;

    use super::{Body, DecodedLength, Incoming, Sender, SizeHint};
    use http_body_util::BodyExt;

    impl Sender {
        async fn ready(&mut self) -> crate::Result<()> {
            futures_util::future::poll_fn(|cx| self.poll_ready(cx)).await
        }

        fn abort(mut self) {
            self.send_error(crate::Error::new_body_write_aborted());
        }
    }

    #[test]
    fn test_size_of() {
        // These are mostly to help catch *accidentally* increasing
        // the size by too much.

        let body_size = mem::size_of::<Incoming>();
        let body_expected_size = mem::size_of::<u64>() * 5;
        assert!(
            body_size <= body_expected_size,
            "Body size = {} <= {}",
            body_size,
            body_expected_size,
        );

        //assert_eq!(body_size, mem::size_of::<Option<Incoming>>(), "Option<Incoming>");

        assert_eq!(
            mem::size_of::<Sender>(),
            mem::size_of::<usize>() * 5,
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
        fn eq(body: Incoming, b: SizeHint, note: &str) {
            let a = body.size_hint();
            assert_eq!(a.lower(), b.lower(), "lower for {:?}", note);
            assert_eq!(a.upper(), b.upper(), "upper for {:?}", note);
        }

        eq(Incoming::empty(), SizeHint::with_exact(0), "empty");

        eq(
            Incoming::channel(DecodedLength::CHUNKED, /*wanter =*/ false).1,
            SizeHint::new(),
            "channel",
        );

        eq(
            Incoming::channel(DecodedLength::new(4), /*wanter =*/ false).1,
            SizeHint::with_exact(4),
            "channel with length",
        );
    }

    #[cfg(not(miri))]
    #[tokio::test]
    async fn channel_abort() {
        let (tx, mut rx) = Incoming::channel(DecodedLength::CHUNKED, /*wanter =*/ false);

        tx.abort();

        let err = rx.frame().await.unwrap().unwrap_err();
        assert!(err.is_body_write_aborted(), "{:?}", err);
    }

    #[cfg(all(not(miri), feature = "http1"))]
    #[tokio::test]
    async fn channel_abort_when_buffer_is_full() {
        let (mut tx, mut rx) = Incoming::channel(DecodedLength::CHUNKED, /*wanter =*/ false);

        tx.try_send_data("chunk 1".into()).expect("send 1");
        // buffer is full, but can still send abort
        tx.abort();

        let chunk1 = rx
            .frame()
            .await
            .expect("item 1")
            .expect("chunk 1")
            .into_data()
            .unwrap();
        assert_eq!(chunk1, "chunk 1");

        let err = rx.frame().await.unwrap().unwrap_err();
        assert!(err.is_body_write_aborted(), "{:?}", err);
    }

    #[cfg(feature = "http1")]
    #[test]
    fn channel_buffers_one() {
        let (mut tx, _rx) = Incoming::channel(DecodedLength::CHUNKED, /*wanter =*/ false);

        tx.try_send_data("chunk 1".into()).expect("send 1");

        // buffer is now full
        let chunk2 = tx.try_send_data("chunk 2".into()).expect_err("send 2");
        assert_eq!(chunk2, "chunk 2");
    }

    #[cfg(not(miri))]
    #[tokio::test]
    async fn channel_empty() {
        let (_, mut rx) = Incoming::channel(DecodedLength::CHUNKED, /*wanter =*/ false);

        assert!(rx.frame().await.is_none());
    }

    #[test]
    fn channel_ready() {
        let (mut tx, _rx) = Incoming::channel(DecodedLength::CHUNKED, /*wanter = */ false);

        let mut tx_ready = tokio_test::task::spawn(tx.ready());

        assert!(tx_ready.poll().is_ready(), "tx is ready immediately");
    }

    #[test]
    fn channel_wanter() {
        let (mut tx, mut rx) = Incoming::channel(DecodedLength::CHUNKED, /*wanter = */ true);

        let mut tx_ready = tokio_test::task::spawn(tx.ready());
        let mut rx_data = tokio_test::task::spawn(rx.frame());

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
        let (mut tx, rx) = Incoming::channel(DecodedLength::CHUNKED, /*wanter = */ true);

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
