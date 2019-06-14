use futures::{Async, Poll, task::Task};

use super::Never;

/// A type to help "yield" a future, such that it is re-scheduled immediately.
///
/// Useful for spin counts, so a future doesn't hog too much time.
#[derive(Debug)]
pub(crate) struct YieldNow {
    cached_task: Option<Task>,
}

impl YieldNow {
    pub(crate) fn new() -> YieldNow {
        YieldNow {
            cached_task: None,
        }
    }

    /// Returns `Ok(Async::NotReady)` always, while also notifying the
    /// current task so that it is rescheduled immediately.
    ///
    /// Since it never returns `Async::Ready` or `Err`, those types are
    /// set to `Never`.
    pub(crate) fn poll_yield(&mut self) -> Poll<Never, Never> {
        // Check for a cached `Task` first...
        if let Some(ref t) = self.cached_task {
            if t.will_notify_current() {
                t.notify();
                return Ok(Async::NotReady);
            }
        }

        // No cached task, or not current, so get a new one...
        let t = ::futures::task::current();
        t.notify();
        self.cached_task = Some(t);
        Ok(Async::NotReady)
    }
}
