use futures_core::Stream;
use futures_channel::{mpsc, oneshot};
use futures_util::future;

use crate::common::{Future, Pin, Poll, task};

pub type RetryPromise<T, U> = oneshot::Receiver<Result<U, (crate::Error, Option<T>)>>;
pub type Promise<T> = oneshot::Receiver<Result<T, crate::Error>>;

pub fn channel<T, U>() -> (Sender<T, U>, Receiver<T, U>) {
    let (tx, rx) = mpsc::unbounded();
    let (giver, taker) = want::new();
    let tx = Sender {
        buffered_once: false,
        giver: giver,
        inner: tx,
    };
    let rx = Receiver {
        inner: rx,
        taker: taker,
    };
    (tx, rx)
}

/// A bounded sender of requests and callbacks for when responses are ready.
///
/// While the inner sender is unbounded, the Giver is used to determine
/// if the Receiver is ready for another request.
pub struct Sender<T, U> {
    /// One message is always allowed, even if the Receiver hasn't asked
    /// for it yet. This boolean keeps track of whether we've sent one
    /// without notice.
    buffered_once: bool,
    /// The Giver helps watch that the the Receiver side has been polled
    /// when the queue is empty. This helps us know when a request and
    /// response have been fully processed, and a connection is ready
    /// for more.
    giver: want::Giver,
    /// Actually bounded by the Giver, plus `buffered_once`.
    inner: mpsc::UnboundedSender<Envelope<T, U>>,
}

/// An unbounded version.
///
/// Cannot poll the Giver, but can still use it to determine if the Receiver
/// has been dropped. However, this version can be cloned.
pub struct UnboundedSender<T, U> {
    /// Only used for `is_closed`, since mpsc::UnboundedSender cannot be checked.
    giver: want::SharedGiver,
    inner: mpsc::UnboundedSender<Envelope<T, U>>,
}

impl<T, U> Sender<T, U> {
    pub fn poll_ready(&mut self, cx: &mut task::Context<'_>) -> Poll<crate::Result<()>> {
        self.giver.poll_want(cx)
            .map_err(|_| crate::Error::new_closed())
    }

    pub fn is_ready(&self) -> bool {
        self.giver.is_wanting()
    }

    pub fn is_closed(&self) -> bool {
        self.giver.is_canceled()
    }

    fn can_send(&mut self) -> bool {
        if self.giver.give() || !self.buffered_once {
            // If the receiver is ready *now*, then of course we can send.
            //
            // If the receiver isn't ready yet, but we don't have anything
            // in the channel yet, then allow one message.
            self.buffered_once = true;
            true
        } else {
            false
        }
    }

    pub fn try_send(&mut self, val: T) -> Result<RetryPromise<T, U>, T> {
        if !self.can_send() {
            return Err(val);
        }
        let (tx, rx) = oneshot::channel();
        self.inner.unbounded_send(Envelope(Some((val, Callback::Retry(tx)))))
            .map(move |_| rx)
            .map_err(|e| e.into_inner().0.take().expect("envelope not dropped").0)
    }

    pub fn send(&mut self, val: T) -> Result<Promise<U>, T> {
        if !self.can_send() {
            return Err(val);
        }
        let (tx, rx) = oneshot::channel();
        self.inner.unbounded_send(Envelope(Some((val, Callback::NoRetry(tx)))))
            .map(move |_| rx)
            .map_err(|e| e.into_inner().0.take().expect("envelope not dropped").0)
    }

    pub fn unbound(self) -> UnboundedSender<T, U> {
        UnboundedSender {
            giver: self.giver.shared(),
            inner: self.inner,
        }
    }
}

impl<T, U> UnboundedSender<T, U> {
    pub fn is_ready(&self) -> bool {
        !self.giver.is_canceled()
    }

    pub fn is_closed(&self) -> bool {
        self.giver.is_canceled()
    }

    pub fn try_send(&mut self, val: T) -> Result<RetryPromise<T, U>, T> {
        let (tx, rx) = oneshot::channel();
        self.inner.unbounded_send(Envelope(Some((val, Callback::Retry(tx)))))
            .map(move |_| rx)
            .map_err(|e| e.into_inner().0.take().expect("envelope not dropped").0)
    }
}

impl<T, U> Clone for UnboundedSender<T, U> {
    fn clone(&self) -> Self {
        UnboundedSender {
            giver: self.giver.clone(),
            inner: self.inner.clone(),
        }
    }
}

pub struct Receiver<T, U> {
    inner: mpsc::UnboundedReceiver<Envelope<T, U>>,
    taker: want::Taker,
}

impl<T, U> Receiver<T, U> {
    pub(crate) fn poll_next(&mut self, cx: &mut task::Context<'_>) -> Poll<Option<(T, Callback<T, U>)>> {
        match Pin::new(&mut self.inner).poll_next(cx) {
            Poll::Ready(item) => Poll::Ready(item.map(|mut env| {
                env.0.take().expect("envelope not dropped")
            })),
            Poll::Pending => {
                self.taker.want();
                Poll::Pending
            },
        }
    }

    pub(crate) fn close(&mut self) {
        self.taker.cancel();
        self.inner.close();
    }

    pub(crate) fn try_recv(&mut self) -> Option<(T, Callback<T, U>)> {
        match self.inner.try_next() {
            Ok(Some(mut env)) => env.0.take(),
            Ok(None) => None,
            Err(_) => None,
        }
    }
}

impl<T, U> Drop for Receiver<T, U> {
    fn drop(&mut self) {
        // Notify the giver about the closure first, before dropping
        // the mpsc::Receiver.
        self.taker.cancel();
    }
}

struct Envelope<T, U>(Option<(T, Callback<T, U>)>);

impl<T, U> Drop for Envelope<T, U> {
    fn drop(&mut self) {
        if let Some((val, cb)) = self.0.take() {
            let _ = cb.send(Err((crate::Error::new_canceled().with("connection closed"), Some(val))));
        }
    }
}

pub enum Callback<T, U> {
    Retry(oneshot::Sender<Result<U, (crate::Error, Option<T>)>>),
    NoRetry(oneshot::Sender<Result<U, crate::Error>>),
}

impl<T, U> Callback<T, U> {
    pub(crate) fn is_canceled(&self) -> bool {
        match *self {
            Callback::Retry(ref tx) => tx.is_canceled(),
            Callback::NoRetry(ref tx) => tx.is_canceled(),
        }
    }

    pub(crate) fn poll_cancel(&mut self, cx: &mut task::Context<'_>) -> Poll<()> {
        match *self {
            Callback::Retry(ref mut tx) => tx.poll_cancel(cx),
            Callback::NoRetry(ref mut tx) => tx.poll_cancel(cx),
        }
    }

    pub(crate) fn send(self, val: Result<U, (crate::Error, Option<T>)>) {
        match self {
            Callback::Retry(tx) => {
                let _ = tx.send(val);
            },
            Callback::NoRetry(tx) => {
                let _ = tx.send(val.map_err(|e| e.0));
            }
        }
    }

    pub(crate) fn send_when(
        self,
        mut when: impl Future<Output=Result<U, (crate::Error, Option<T>)>> + Unpin,
    ) -> impl Future<Output=()> {
        let mut cb = Some(self);

        // "select" on this callback being canceled, and the future completing
        future::poll_fn(move |cx| {
            match Pin::new(&mut when).poll(cx) {
                Poll::Ready(Ok(res)) => {
                    cb.take()
                        .expect("polled after complete")
                        .send(Ok(res));
                    Poll::Ready(())
                },
                Poll::Pending => {
                    // check if the callback is canceled
                    ready!(cb.as_mut().unwrap().poll_cancel(cx));
                    trace!("send_when canceled");
                    Poll::Ready(())
                },
                Poll::Ready(Err(err)) => {
                    cb.take()
                        .expect("polled after complete")
                        .send(Err(err));
                    Poll::Ready(())
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    // FIXME: re-implement tests with `async/await`, this import should
    // trigger a warning to remind us
    use crate::Error;
    /*
    #[cfg(feature = "nightly")]
    extern crate test;

    use futures::{future, Future, Stream};


    #[derive(Debug)]
    struct Custom(i32);

    #[test]
    fn drop_receiver_sends_cancel_errors() {
        let _ = pretty_env_logger::try_init();

        future::lazy(|| {
            let (mut tx, mut rx) = super::channel::<Custom, ()>();

            // must poll once for try_send to succeed
            assert!(rx.poll().expect("rx empty").is_not_ready());

            let promise = tx.try_send(Custom(43)).unwrap();
            drop(rx);

            promise.then(|fulfilled| {
                let err = fulfilled
                    .expect("fulfilled")
                    .expect_err("promise should error");

                match (err.0.kind(), err.1) {
                    (&crate::error::Kind::Canceled, Some(_)) => (),
                    e => panic!("expected Error::Cancel(_), found {:?}", e),
                }

                Ok::<(), ()>(())
            })
        }).wait().unwrap();
    }

    #[test]
    fn sender_checks_for_want_on_send() {
        future::lazy(|| {
            let (mut tx, mut rx) = super::channel::<Custom, ()>();
            // one is allowed to buffer, second is rejected
            let _ = tx.try_send(Custom(1)).expect("1 buffered");
            tx.try_send(Custom(2)).expect_err("2 not ready");

            assert!(rx.poll().expect("rx 1").is_ready());
            // Even though 1 has been popped, only 1 could be buffered for the
            // lifetime of the channel.
            tx.try_send(Custom(2)).expect_err("2 still not ready");

            assert!(rx.poll().expect("rx empty").is_not_ready());
            let _ = tx.try_send(Custom(2)).expect("2 ready");

            Ok::<(), ()>(())
        }).wait().unwrap();
    }

    #[test]
    fn unbounded_sender_doesnt_bound_on_want() {
        let (tx, rx) = super::channel::<Custom, ()>();
        let mut tx = tx.unbound();

        let _ = tx.try_send(Custom(1)).unwrap();
        let _ = tx.try_send(Custom(2)).unwrap();
        let _ = tx.try_send(Custom(3)).unwrap();

        drop(rx);

        let _ = tx.try_send(Custom(4)).unwrap_err();
    }

    #[cfg(feature = "nightly")]
    #[bench]
    fn giver_queue_throughput(b: &mut test::Bencher) {
        use crate::{Body, Request, Response};
        let (mut tx, mut rx) = super::channel::<Request<Body>, Response<Body>>();

        b.iter(move || {
            ::futures::future::lazy(|| {
                let _ = tx.send(Request::default()).unwrap();
                loop {
                    let ok = rx.poll().unwrap();
                    if ok.is_not_ready() {
                        break;
                    }
                }


                Ok::<_, ()>(())
            }).wait().unwrap();
        })
    }

    #[cfg(feature = "nightly")]
    #[bench]
    fn giver_queue_not_ready(b: &mut test::Bencher) {
        let (_tx, mut rx) = super::channel::<i32, ()>();

        b.iter(move || {
            ::futures::future::lazy(|| {
                assert!(rx.poll().unwrap().is_not_ready());

                Ok::<(), ()>(())
            }).wait().unwrap();
        })
    }

    #[cfg(feature = "nightly")]
    #[bench]
    fn giver_queue_cancel(b: &mut test::Bencher) {
        let (_tx, mut rx) = super::channel::<i32, ()>();

        b.iter(move || {
            rx.taker.cancel();
        })
    }
    */
}
