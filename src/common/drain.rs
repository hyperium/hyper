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

