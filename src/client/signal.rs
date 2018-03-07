use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use futures::{Async, Poll};
use futures::task::{self, Task};

use self::lock::Lock;

pub fn new() -> (Giver, Taker) {
    let inner = Arc::new(Inner {
        state: AtomicUsize::new(STATE_IDLE),
        task: Lock::new(None),
    });
    let inner2 = inner.clone();
    (
        Giver {
            inner: inner,
        },
        Taker {
            inner: inner2,
        },
    )
}

#[derive(Clone)]
pub struct Giver {
    inner: Arc<Inner>,
}

pub struct Taker {
    inner: Arc<Inner>,
}

const STATE_IDLE: usize = 0;
const STATE_WANT: usize = 1;
const STATE_GIVE: usize = 2;
const STATE_CLOSED: usize = 3;

struct Inner {
    state: AtomicUsize,
    task: Lock<Option<Task>>,
}

impl Giver {
    pub fn poll_want(&mut self) -> Poll<(), ()> {
        loop {
            let state = self.inner.state.load(Ordering::SeqCst);
            match state {
                STATE_WANT => {
                    // only set to IDLE if it is still Want
                    self.inner.state.compare_and_swap(
                        STATE_WANT,
                        STATE_IDLE,
                        Ordering::SeqCst,
                    );
                    return Ok(Async::Ready(()))
                },
                STATE_GIVE => {
                    // we're already waiting, return
                    return Ok(Async::NotReady)
                }
                STATE_CLOSED => return Err(()),
                // Taker doesn't want anything yet, so park.
                _ => {
                    if let Some(mut locked) = self.inner.task.try_lock() {

                        // While we have the lock, try to set to GIVE.
                        let old = self.inner.state.compare_and_swap(
                            STATE_IDLE,
                            STATE_GIVE,
                            Ordering::SeqCst,
                        );
                        // If it's not still IDLE, something happened!
                        // Go around the loop again.
                        if old == STATE_IDLE {
                            *locked = Some(task::current());
                            return Ok(Async::NotReady)
                        }
                    } else {
                        // if we couldn't take the lock, then a Taker has it.
                        // The *ONLY* reason is because it is in the process of notifying us
                        // of its want.
                        //
                        // We need to loop again to see what state it was changed to.
                    }
                },
            }
        }
    }

    pub fn is_canceled(&self) -> bool {
        self.inner.state.load(Ordering::SeqCst) == STATE_CLOSED
    }
}

impl Taker {
    pub fn cancel(&self) {
        self.signal(STATE_CLOSED)
    }

    pub fn want(&self) {
        self.signal(STATE_WANT)
    }

    fn signal(&self, state: usize) {
        let old_state = self.inner.state.swap(state, Ordering::SeqCst);
        match old_state {
            STATE_WANT | STATE_CLOSED | STATE_IDLE => (),
            _ => {
                loop {
                    if let Some(mut locked) = self.inner.task.try_lock() {
                        if let Some(task) = locked.take() {
                            task.notify();
                        }
                        return;
                    } else {
                        // if we couldn't take the lock, then a Giver has it.
                        // The *ONLY* reason is because it is in the process of parking.
                        //
                        // We need to loop and take the lock so we can notify this task.
                    }
                }
            },
        }
    }
}

impl Drop for Taker {
    fn drop(&mut self) {
        self.cancel();
    }
}


// a sub module just to protect unsafety
mod lock {
    use std::cell::UnsafeCell;
    use std::ops::{Deref, DerefMut};
    use std::sync::atomic::{AtomicBool, Ordering};

    pub struct Lock<T> {
        is_locked: AtomicBool,
        value: UnsafeCell<T>,
    }

    impl<T> Lock<T> {
        pub fn new(val: T) -> Lock<T> {
            Lock {
                is_locked: AtomicBool::new(false),
                value: UnsafeCell::new(val),
            }
        }

        pub fn try_lock(&self) -> Option<Locked<T>> {
            if !self.is_locked.swap(true, Ordering::SeqCst) {
                Some(Locked { lock: self })
            } else {
                None
            }
        }
    }

    unsafe impl<T: Send> Send for Lock<T> {}
    unsafe impl<T: Send> Sync for Lock<T> {}

    pub struct Locked<'a, T: 'a> {
        lock: &'a Lock<T>,
    }

    impl<'a, T> Deref for Locked<'a, T> {
        type Target = T;
        fn deref(&self) -> &T {
            unsafe { &*self.lock.value.get() }
        }
    }

    impl<'a, T> DerefMut for Locked<'a, T> {
        fn deref_mut(&mut self) -> &mut T {
            unsafe { &mut *self.lock.value.get() }
        }
    }

    impl<'a, T> Drop for Locked<'a, T> {
        fn drop(&mut self) {
            self.lock.is_locked.store(false, Ordering::SeqCst);
        }
    }
}
