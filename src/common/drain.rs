use std::mem;

use pin_project::pin_project;
use tokio::sync::{mpsc, watch};

use super::{task, Future, Never, Pin, Poll};

// Sentinel value signaling that the watch is still open
#[derive(Clone, Copy)]
enum Action {
    Open,
    // Closed isn't sent via the `Action` type, but rather once
    // the watch::Sender is dropped.
}

pub fn channel() -> (Signal, Watch) {
    let (tx, rx) = watch::channel(Action::Open);
    let (drained_tx, drained_rx) = mpsc::channel(1);
    (
        Signal {
            drained_rx,
            _tx: tx,
        },
        Watch { drained_tx, rx },
    )
}

pub struct Signal {
    drained_rx: mpsc::Receiver<Never>,
    _tx: watch::Sender<Action>,
}

pub struct Draining {
    drained_rx: mpsc::Receiver<Never>,
}

#[derive(Clone)]
pub struct Watch {
    drained_tx: mpsc::Sender<Never>,
    rx: watch::Receiver<Action>,
}

#[allow(missing_debug_implementations)]
#[pin_project]
pub struct Watching<F, FN> {
    #[pin]
    future: F,
    state: State<FN>,
    watch: Watch,
}

enum State<F> {
    Watch(F),
    Draining,
}

impl Signal {
    pub fn drain(self) -> Draining {
        // Simply dropping `self.tx` will signal the watchers
        Draining {
            drained_rx: self.drained_rx,
        }
    }
}

impl Future for Draining {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Self::Output> {
        match ready!(self.drained_rx.poll_recv(cx)) {
            Some(never) => match never {},
            None => Poll::Ready(()),
        }
    }
}

impl Watch {
    pub fn watch<F, FN>(self, future: F, on_drain: FN) -> Watching<F, FN>
    where
        F: Future,
        FN: FnOnce(Pin<&mut F>),
    {
        Watching {
            future,
            state: State::Watch(on_drain),
            watch: self,
        }
    }
}

impl<F, FN> Future for Watching<F, FN>
where
    F: Future,
    FN: FnOnce(Pin<&mut F>),
{
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Self::Output> {
        let mut me = self.project();
        loop {
            match mem::replace(me.state, State::Draining) {
                State::Watch(on_drain) => {
                    match me.watch.rx.poll_recv_ref(cx) {
                        Poll::Ready(None) => {
                            // Drain has been triggered!
                            on_drain(me.future.as_mut());
                        }
                        Poll::Ready(Some(_ /*State::Open*/)) | Poll::Pending => {
                            *me.state = State::Watch(on_drain);
                            return me.future.poll(cx);
                        }
                    }
                }
                State::Draining => return me.future.poll(cx),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestMe {
        draining: bool,
        finished: bool,
        poll_cnt: usize,
    }

    impl Future for TestMe {
        type Output = ();

        fn poll(mut self: Pin<&mut Self>, _: &mut task::Context<'_>) -> Poll<Self::Output> {
            self.poll_cnt += 1;
            if self.finished {
                Poll::Ready(())
            } else {
                Poll::Pending
            }
        }
    }

    #[test]
    fn watch() {
        let mut mock = tokio_test::task::spawn(());
        mock.enter(|cx, _| {
            let (tx, rx) = channel();
            let fut = TestMe {
                draining: false,
                finished: false,
                poll_cnt: 0,
            };

            let mut watch = rx.watch(fut, |mut fut| {
                fut.draining = true;
            });

            assert_eq!(watch.future.poll_cnt, 0);

            // First poll should poll the inner future
            assert!(Pin::new(&mut watch).poll(cx).is_pending());
            assert_eq!(watch.future.poll_cnt, 1);

            // Second poll should poll the inner future again
            assert!(Pin::new(&mut watch).poll(cx).is_pending());
            assert_eq!(watch.future.poll_cnt, 2);

            let mut draining = tx.drain();
            // Drain signaled, but needs another poll to be noticed.
            assert!(!watch.future.draining);
            assert_eq!(watch.future.poll_cnt, 2);

            // Now, poll after drain has been signaled.
            assert!(Pin::new(&mut watch).poll(cx).is_pending());
            assert_eq!(watch.future.poll_cnt, 3);
            assert!(watch.future.draining);

            // Draining is not ready until watcher completes
            assert!(Pin::new(&mut draining).poll(cx).is_pending());

            // Finishing up the watch future
            watch.future.finished = true;
            assert!(Pin::new(&mut watch).poll(cx).is_ready());
            assert_eq!(watch.future.poll_cnt, 4);
            drop(watch);

            assert!(Pin::new(&mut draining).poll(cx).is_ready());
        })
    }

    #[test]
    fn watch_clones() {
        let mut mock = tokio_test::task::spawn(());
        mock.enter(|cx, _| {
            let (tx, rx) = channel();

            let fut1 = TestMe {
                draining: false,
                finished: false,
                poll_cnt: 0,
            };
            let fut2 = TestMe {
                draining: false,
                finished: false,
                poll_cnt: 0,
            };

            let watch1 = rx.clone().watch(fut1, |mut fut| {
                fut.draining = true;
            });
            let watch2 = rx.watch(fut2, |mut fut| {
                fut.draining = true;
            });

            let mut draining = tx.drain();

            // Still 2 outstanding watchers
            assert!(Pin::new(&mut draining).poll(cx).is_pending());

            // drop 1 for whatever reason
            drop(watch1);

            // Still not ready, 1 other watcher still pending
            assert!(Pin::new(&mut draining).poll(cx).is_pending());

            drop(watch2);

            // Now all watchers are gone, draining is complete
            assert!(Pin::new(&mut draining).poll(cx).is_ready());
        });
    }
}
