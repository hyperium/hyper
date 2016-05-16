use std::fmt;
use std::sync::{Arc, mpsc};
use std::sync::atomic::{AtomicBool, Ordering};
use ::rotor;

pub use std::sync::mpsc::TryRecvError;

pub fn new<T>(notify: rotor::Notifier) -> (Sender<T>, Receiver<T>) {
    let b = Arc::new(AtomicBool::new(false));
    let (tx, rx) = mpsc::channel();
    (Sender {
        awake: b.clone(),
        notify: notify,
        tx: tx,
    },
    Receiver {
        awake: b,
        rx: rx,
    })
}

pub fn share<T, U>(other: &Sender<U>) -> (Sender<T>, Receiver<T>) {
    let (tx, rx) = mpsc::channel();
    let notify = other.notify.clone();
    let b = other.awake.clone();
    (Sender {
        awake: b.clone(),
        notify: notify,
        tx: tx,
    },
    Receiver {
        awake: b,
        rx: rx,
    })
}

pub struct Sender<T> {
    awake: Arc<AtomicBool>,
    notify: rotor::Notifier,
    tx: mpsc::Sender<T>,
}

impl<T: Send> Sender<T> {
    pub fn send(&self, val: T) -> Result<(), SendError<T>> {
        try!(self.tx.send(val));
        if !self.awake.swap(true, Ordering::SeqCst) {
            try!(self.notify.wakeup());
        }
        Ok(())
    }
}

impl<T> Clone for Sender<T> {
    fn clone(&self) -> Sender<T> {
        Sender {
            awake: self.awake.clone(),
            notify: self.notify.clone(),
            tx: self.tx.clone(),
        }
    }
}

impl<T> fmt::Debug for Sender<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Sender")
            .field("notify", &self.notify)
            .finish()
    }
}

#[derive(Debug)]
pub struct SendError<T>(pub Option<T>);

impl<T> From<mpsc::SendError<T>> for SendError<T> {
    fn from(e: mpsc::SendError<T>) -> SendError<T> {
        SendError(Some(e.0))
    }
}

impl<T> From<rotor::WakeupError> for SendError<T> {
    fn from(_e: rotor::WakeupError) -> SendError<T> {
        SendError(None)
    }
}

pub struct Receiver<T> {
    awake: Arc<AtomicBool>,
    rx: mpsc::Receiver<T>,
}

impl<T: Send> Receiver<T> {
    pub fn try_recv(&self) -> Result<T, mpsc::TryRecvError> {
        self.awake.store(false, Ordering::Relaxed);
        self.rx.try_recv()
    }
}
