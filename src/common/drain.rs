use std::mem;

use futures::{Async, Future, Poll, Stream};
use futures::future::Shared;
use futures::sync::{mpsc, oneshot};

use super::Never;

pub fn channel() -> (Signal, Watch) {
    let (tx, rx) = oneshot::channel();
    let (drained_tx, drained_rx) = mpsc::channel(0);
    (
        Signal {
            drained_rx,
            tx,
        },
        Watch {
            drained_tx,
            rx: rx.shared(),
        },
    )
}

pub struct Signal {
    drained_rx: mpsc::Receiver<Never>,
    tx: oneshot::Sender<()>,
}

pub struct Draining {
    drained_rx: mpsc::Receiver<Never>,
}

#[derive(Clone)]
pub struct Watch {
    drained_tx: mpsc::Sender<Never>,
    rx: Shared<oneshot::Receiver<()>>,
}

#[allow(missing_debug_implementations)]
pub struct Watching<F, FN> {
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
        let _ = self.tx.send(());
        Draining {
            drained_rx: self.drained_rx,
        }
    }
}

impl Future for Draining {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        match try_ready!(self.drained_rx.poll()) {
            Some(never) => match never {},
            None => Ok(Async::Ready(())),
        }
    }
}

impl Watch {
    pub fn watch<F, FN>(self, future: F, on_drain: FN) -> Watching<F, FN>
    where
        F: Future,
        FN: FnOnce(&mut F),
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
    FN: FnOnce(&mut F),
{
    type Item = F::Item;
    type Error = F::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        loop {
            match mem::replace(&mut self.state, State::Draining) {
                State::Watch(on_drain) => {
                    match self.watch.rx.poll() {
                        Ok(Async::Ready(_)) | Err(_) => {
                            // Drain has been triggered!
                            on_drain(&mut self.future);
                        },
                        Ok(Async::NotReady) => {
                            self.state = State::Watch(on_drain);
                            return self.future.poll();
                        },
                    }
                },
                State::Draining => {
                    return self.future.poll();
                },
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use futures::{future, Async, Future, Poll};
    use super::*;

    struct TestMe {
        draining: bool,
        finished: bool,
        poll_cnt: usize,
    }

    impl Future for TestMe {
        type Item = ();
        type Error = ();

        fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
            self.poll_cnt += 1;
            if self.finished {
                Ok(Async::Ready(()))
            } else {
                Ok(Async::NotReady)
            }
        }
    }

    #[test]
    fn watch() {
        future::lazy(|| {
            let (tx, rx) = channel();
            let fut = TestMe {
                draining: false,
                finished: false,
                poll_cnt: 0,
            };

            let mut watch = rx.watch(fut, |fut| {
                fut.draining = true;
            });

            assert_eq!(watch.future.poll_cnt, 0);

            // First poll should poll the inner future
            assert!(watch.poll().unwrap().is_not_ready());
            assert_eq!(watch.future.poll_cnt, 1);

            // Second poll should poll the inner future again
            assert!(watch.poll().unwrap().is_not_ready());
            assert_eq!(watch.future.poll_cnt, 2);

            let mut draining = tx.drain();
            // Drain signaled, but needs another poll to be noticed.
            assert!(!watch.future.draining);
            assert_eq!(watch.future.poll_cnt, 2);

            // Now, poll after drain has been signaled.
            assert!(watch.poll().unwrap().is_not_ready());
            assert_eq!(watch.future.poll_cnt, 3);
            assert!(watch.future.draining);

            // Draining is not ready until watcher completes
            assert!(draining.poll().unwrap().is_not_ready());

            // Finishing up the watch future
            watch.future.finished = true;
            assert!(watch.poll().unwrap().is_ready());
            assert_eq!(watch.future.poll_cnt, 4);
            drop(watch);

            assert!(draining.poll().unwrap().is_ready());

            Ok::<_, ()>(())
        }).wait().unwrap();
    }

    #[test]
    fn watch_clones() {
        future::lazy(|| {
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

            let watch1 = rx.clone().watch(fut1, |fut| {
                fut.draining = true;
            });
            let watch2 = rx.watch(fut2, |fut| {
                fut.draining = true;
            });

            let mut draining = tx.drain();

            // Still 2 outstanding watchers
            assert!(draining.poll().unwrap().is_not_ready());

            // drop 1 for whatever reason
            drop(watch1);

            // Still not ready, 1 other watcher still pending
            assert!(draining.poll().unwrap().is_not_ready());

            drop(watch2);

            // Now all watchers are gone, draining is complete
            assert!(draining.poll().unwrap().is_ready());

            Ok::<_, ()>(())
        }).wait().unwrap();
    }
}

