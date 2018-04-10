use futures::{Async, Poll, Stream};
use futures::sync::{mpsc, oneshot};
use want;

use common::Never;

//pub type Callback<T, U> = oneshot::Sender<Result<U, (::Error, Option<T>)>>;
pub type RetryPromise<T, U> = oneshot::Receiver<Result<U, (::Error, Option<T>)>>;
pub type Promise<T> = oneshot::Receiver<Result<T, ::Error>>;

pub fn channel<T, U>() -> (Sender<T, U>, Receiver<T, U>) {
    let (tx, rx) = mpsc::channel(0);
    let (giver, taker) = want::new();
    let tx = Sender {
        giver: giver,
        inner: tx,
    };
    let rx = Receiver {
        inner: rx,
        taker: taker,
    };
    (tx, rx)
}

pub struct Sender<T, U> {
    // The Giver helps watch that the the Receiver side has been polled
    // when the queue is empty. This helps us know when a request and
    // response have been fully processed, and a connection is ready
    // for more.
    giver: want::Giver,
    //inner: mpsc::Sender<(T, Callback<T, U>)>,
    inner: mpsc::Sender<Envelope<T, U>>,
}

impl<T, U> Sender<T, U> {
    pub fn poll_ready(&mut self) -> Poll<(), ::Error> {
        match self.inner.poll_ready() {
            Ok(Async::Ready(())) => {
                // there's room in the queue, but does the Connection
                // want a message yet?
                self.giver.poll_want()
                    .map_err(|_| ::Error::Closed)
            },
            Ok(Async::NotReady) => Ok(Async::NotReady),
            Err(_) => Err(::Error::Closed),
        }
    }

    pub fn is_ready(&self) -> bool {
        self.giver.is_wanting()
    }

    pub fn is_closed(&self) -> bool {
        self.giver.is_canceled()
    }

    pub fn try_send(&mut self, val: T) -> Result<RetryPromise<T, U>, T> {
        let (tx, rx) = oneshot::channel();
        self.inner.try_send(Envelope(Some((val, Callback::Retry(tx)))))
            .map(move |_| rx)
            .map_err(|e| e.into_inner().0.take().expect("envelope not dropped").0)
    }

    pub fn send(&mut self, val: T) -> Result<Promise<U>, T> {
        let (tx, rx) = oneshot::channel();
        self.inner.try_send(Envelope(Some((val, Callback::NoRetry(tx)))))
            .map(move |_| rx)
            .map_err(|e| e.into_inner().0.take().expect("envelope not dropped").0)
    }
}

pub struct Receiver<T, U> {
    //inner: mpsc::Receiver<(T, Callback<T, U>)>,
    inner: mpsc::Receiver<Envelope<T, U>>,
    taker: want::Taker,
}

impl<T, U> Stream for Receiver<T, U> {
    type Item = (T, Callback<T, U>);
    type Error = Never;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        match self.inner.poll() {
            Ok(Async::Ready(item)) => Ok(Async::Ready(item.map(|mut env| {
                env.0.take().expect("envelope not dropped")
            }))),
            Ok(Async::NotReady) => {
                self.taker.want();
                Ok(Async::NotReady)
            }
            Err(()) => unreachable!("mpsc never errors"),
        }
    }
}

/*
TODO: with futures 0.2, bring this Drop back and toss Envelope

The problem is, there is a bug in futures 0.1 mpsc channel, where
even though you may call `rx.close()`, `rx.poll()` may still think
there are messages and so should park the current task. In futures
0.2, we can use `try_next`, and not even risk such a bug.

For now, use an `Envelope` that has this drop guard logic instead.

impl<T, U> Drop for Receiver<T, U> {
    fn drop(&mut self) {
        self.taker.cancel();
        self.inner.close();

        // This poll() is safe to call in `Drop`, because we've
        // called, `close`, which promises that no new messages
        // will arrive, and thus, once we reach the end, we won't
        // see a `NotReady` (and try to park), but a Ready(None).
        //
        // All other variants:
        // - Ready(None): the end. we want to stop looping
        // - NotReady: unreachable
        // - Err: unreachable
        while let Ok(Async::Ready(Some((val, cb)))) = self.inner.poll() {
            let _ = cb.send(Err((::Error::new_canceled(None::<::Error>), Some(val))));
        }
    }

}
*/

struct Envelope<T, U>(Option<(T, Callback<T, U>)>);

impl<T, U> Drop for Envelope<T, U> {
    fn drop(&mut self) {
        if let Some((val, cb)) = self.0.take() {
            let _ = cb.send(Err((::Error::new_canceled(None::<::Error>), Some(val))));
        }
    }
}

pub enum Callback<T, U> {
    Retry(oneshot::Sender<Result<U, (::Error, Option<T>)>>),
    NoRetry(oneshot::Sender<Result<U, ::Error>>),
}

impl<T, U> Callback<T, U> {
    pub fn poll_cancel(&mut self) -> Poll<(), ()> {
        match *self {
            Callback::Retry(ref mut tx) => tx.poll_cancel(),
            Callback::NoRetry(ref mut tx) => tx.poll_cancel(),
        }
    }

    pub fn send(self, val: Result<U, (::Error, Option<T>)>) {
        match self {
            Callback::Retry(tx) => {
                let _ = tx.send(val);
            },
            Callback::NoRetry(tx) => {
                let _ = tx.send(val.map_err(|e| e.0));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    extern crate pretty_env_logger;
    #[cfg(feature = "nightly")]
    extern crate test;

    use futures::{future, Future};

    #[cfg(feature = "nightly")]
    use futures::{Stream};

    #[test]
    fn drop_receiver_sends_cancel_errors() {
        let _ = pretty_env_logger::try_init();

        future::lazy(|| {
            #[derive(Debug)]
            struct Custom(i32);
            let (mut tx, rx) = super::channel::<Custom, ()>();

            let promise = tx.try_send(Custom(43)).unwrap();
            drop(rx);

            promise.then(|fulfilled| {
                let res = fulfilled.expect("fulfilled");
                match res.unwrap_err() {
                    (::Error::Cancel(_), Some(_)) => (),
                    e => panic!("expected Error::Cancel(_), found {:?}", e),
                }

                Ok::<(), ()>(())
            })
        }).wait().unwrap();
    }

    #[cfg(feature = "nightly")]
    #[bench]
    fn giver_queue_throughput(b: &mut test::Bencher) {
        let (mut tx, mut rx) = super::channel::<i32, ()>();

        b.iter(move || {
            ::futures::future::lazy(|| {
                let _ = tx.send(1).unwrap();
                loop {
                    let async = rx.poll().unwrap();
                    if async.is_not_ready() {
                        break;
                    }
                }


                Ok::<(), ()>(())
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
}
