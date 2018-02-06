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

//TODO: Drop for Receiver should consume inner

#[cfg(test)]
mod tests {

    #[cfg(feature = "nightly")]
    extern crate test;

    #[cfg(feature = "nightly")]
    use futures::{Future, Stream};

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
