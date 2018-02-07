use futures::{Async, Future, Poll, Stream};
use futures::sync::{mpsc, oneshot};

use common::Never;
use super::cancel::{Cancel, Canceled};

pub type Callback<U> = oneshot::Sender<::Result<U>>;
pub type Promise<U> = oneshot::Receiver<::Result<U>>;

pub fn channel<T, U>() -> (Sender<T, U>, Receiver<T, U>) {
    let (tx, rx) = mpsc::unbounded();
    let (cancel, canceled) = Cancel::new();
    let tx = Sender {
        cancel: cancel,
        inner: tx,
    };
    let rx = Receiver {
        canceled: canceled,
        inner: rx,
    };
    (tx, rx)
}

pub struct Sender<T, U> {
    cancel: Cancel,
    inner: mpsc::UnboundedSender<(T, Callback<U>)>,
}

impl<T, U> Sender<T, U> {
    pub fn is_closed(&self) -> bool {
        self.cancel.is_canceled()
    }

    pub fn cancel(&self) {
        self.cancel.cancel();
    }

    pub fn send(&self, val: T) -> Result<Promise<U>, T> {
        let (tx, rx) = oneshot::channel();
        self.inner.unbounded_send((val, tx))
            .map(move |_| rx)
            .map_err(|e| e.into_inner().0)
    }
}

impl<T, U> Clone for Sender<T, U> {
    fn clone(&self) -> Sender<T, U> {
        Sender {
            cancel: self.cancel.clone(),
            inner: self.inner.clone(),
        }
    }
}

pub struct Receiver<T, U> {
    canceled: Canceled,
    inner: mpsc::UnboundedReceiver<(T, Callback<U>)>,
}

impl<T, U> Stream for Receiver<T, U> {
    type Item = (T, Callback<U>);
    type Error = Never;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        if let Async::Ready(()) = self.canceled.poll()? {
            return Ok(Async::Ready(None));
        }
        self.inner.poll().map_err(|()| unreachable!("mpsc never errors"))
    }
}

impl<T, U> Drop for Receiver<T, U> {
    fn drop(&mut self) {
        self.canceled.cancel();
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
        while let Ok(Async::Ready(Some((_val, cb)))) = self.inner.poll() {
            // maybe in future, we pass the value along with the error?
            let _ = cb.send(Err(::Error::new_canceled()));
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
            let (tx, rx) = super::channel::<Custom, ()>();

            let promise = tx.send(Custom(43)).unwrap();
            drop(rx);

            promise.then(|fulfilled| {
                let res = fulfilled.expect("fulfilled");
                match res.unwrap_err() {
                    ::Error::Cancel(_) => (),
                    e => panic!("expected Error::Cancel(_), found {:?}", e),
                }

                Ok::<(), ()>(())
            })
        }).wait().unwrap();
    }

    #[cfg(feature = "nightly")]
    #[bench]
    fn cancelable_queue_throughput(b: &mut test::Bencher) {
        let (tx, mut rx) = super::channel::<i32, ()>();

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
    fn cancelable_queue_not_ready(b: &mut test::Bencher) {
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
    fn cancelable_queue_cancel(b: &mut test::Bencher) {
        let (tx, _rx) = super::channel::<i32, ()>();

        b.iter(move || {
            tx.cancel();
        })
    }
}
