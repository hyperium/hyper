use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use futures::{Async, Future, Poll};
use futures::task::{self, Task};

use common::Never;

use self::lock::Lock;

#[derive(Clone)]
pub struct Cancel {
    inner: Arc<Inner>,
}

pub struct Canceled {
    inner: Arc<Inner>,
}

struct Inner {
    is_canceled: AtomicBool,
    task: Lock<Option<Task>>,
}

impl Cancel {
    pub fn new() -> (Cancel, Canceled) {
        let inner = Arc::new(Inner {
            is_canceled: AtomicBool::new(false),
            task: Lock::new(None),
        });
        let inner2 = inner.clone();
        (
            Cancel {
                inner: inner,
            },
            Canceled {
                inner: inner2,
            },
        )
    }

    pub fn cancel(&self) {
        if !self.inner.is_canceled.swap(true, Ordering::SeqCst) {
            if let Some(mut locked) = self.inner.task.try_lock() {
                if let Some(task) = locked.take() {
                    task.notify();
                }
            }
            // if we couldn't take the lock, Canceled was trying to park.
            // After parking, it will check is_canceled one last time,
            // so we can just stop here.
        }
    }

    pub fn is_canceled(&self) -> bool {
        self.inner.is_canceled.load(Ordering::SeqCst)
    }
}

impl Future for Canceled {
    type Item = ();
    type Error = Never;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        if self.inner.is_canceled.load(Ordering::SeqCst) {
            Ok(Async::Ready(()))
        } else {
            if let Some(mut locked) = self.inner.task.try_lock() {
                if locked.is_none() {
                    // it's possible a Cancel just tried to cancel on another thread,
                    // and we just missed it. Once we have the lock, we should check
                    // one more time before parking this task and going away.
                    if self.inner.is_canceled.load(Ordering::SeqCst) {
                        return Ok(Async::Ready(()));
                    }
                    *locked = Some(task::current());
                }
                Ok(Async::NotReady)
            } else {
                // if we couldn't take the lock, then a Cancel taken has it.
                // The *ONLY* reason is because it is in the process of canceling.
                Ok(Async::Ready(()))
            }
        }
    }
}

impl Drop for Canceled {
    fn drop(&mut self) {
        self.inner.is_canceled.store(true, Ordering::SeqCst);
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
