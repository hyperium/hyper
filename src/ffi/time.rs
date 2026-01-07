use std::collections::binary_heap::{BinaryHeap, PeekMut};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};
use std::time::{Duration, Instant};

/// A heap of timer entries with their associated wakers, backing `TimerFuture` instances.
pub(super) struct TimerHeap(BinaryHeap<TimerEntry>);

/// The entry in the timer heap for a programmed timer.  The heap should expire the timer at
/// `wake_at` and wake any waker it finds in the `shared` state.
struct TimerEntry {
    shared: Arc<Mutex<TimerShared>>,
    wake_at: Instant,
}

/// A future that completes at `wake_at`.  Requires that the associated `TimerHeap` is driven
/// in order to make progress.
struct TimerFuture {
    heap: Arc<Mutex<TimerHeap>>,
    wake_at: Instant,
    // This is None when the timer isn't programmed in the heap
    shared: Option<Arc<Mutex<TimerShared>>>,
}

/// Shared between the timer future and the timer heap.  If the heap expires a timer it should wake
/// the associated waker if one is present (if not, that indicates that the timer has been cancelled
/// and can be discarded).
struct TimerShared {
    waker: Option<Waker>,
}

// ===== impl TimerEntry =====

// Consistency with `Ord` requires us to report `TimerEntry`s with the same `wake_at` as equal.
impl std::cmp::PartialEq for TimerEntry {
    fn eq(&self, other: &Self) -> bool {
        self.wake_at.eq(&other.wake_at)
    }
}
impl std::cmp::Eq for TimerEntry {}

// BinaryHeap is a max-heap and we want the top of the heap to be the nearest to popping timer
// so we want the "bigger" timer to have the earlier `wake_at` time.  We achieve this by flipping
// the sides of the comparisons in `Ord` and implementing `PartialOrd` in terms of `Ord`.
impl std::cmp::PartialOrd for TimerEntry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl std::cmp::Ord for TimerEntry {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Note flipped order
        other.wake_at.cmp(&self.wake_at)
    }
}

// ===== impl TimerFuture =====

impl std::future::Future for TimerFuture {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let now = Instant::now();

        if self.wake_at <= now {
            return Poll::Ready(());
        }

        match &self.shared {
            Some(shared) => {
                // Timer was already programmed, update the waker
                shared.lock().unwrap().waker = Some(cx.waker().clone());
            }
            None => {
                // Need to program the timer into the heap
                let shared = Arc::new(Mutex::new(TimerShared {
                    waker: Some(cx.waker().clone()),
                }));
                {
                    let mut heap = self.heap.lock().unwrap();
                    let t = TimerEntry {
                        shared: Arc::clone(&shared),
                        wake_at: self.wake_at,
                    };
                    heap.0.push(t);
                }
                self.shared = Some(shared);
            }
        }

        return Poll::Pending;
    }
}

impl std::ops::Drop for TimerFuture {
    fn drop(&mut self) {
        if let Some(shared) = &self.shared {
            let _ = shared.lock().unwrap().waker.take();
        }
    }
}

// ===== impl TimerHeap =====

impl crate::rt::Timer for Arc<Mutex<TimerHeap>> {
    fn sleep(&self, duration: Duration) -> Pin<Box<dyn crate::rt::Sleep>> {
        self.sleep_until(Instant::now() + duration)
    }

    fn sleep_until(&self, instant: Instant) -> Pin<Box<dyn crate::rt::Sleep>> {
        Box::pin(TimerFuture {
            heap: Arc::clone(self),
            wake_at: instant,
            shared: None,
        })
    }
}

impl TimerHeap {
    pub(super) fn new() -> Self {
        Self(BinaryHeap::new())
    }

    /// Walk the timer heap waking active timers and discarding cancelled ones.
    pub(super) fn process_timers(&mut self) {
        let now = Instant::now();
        while let Some(timer) = self.0.peek_mut() {
            if let Some(waker) = &mut timer.shared.lock().unwrap().waker {
                if timer.wake_at < now {
                    waker.wake_by_ref();
                } else {
                    break;
                }
            }
            // This time was for the past so pop it now.
            let _ = PeekMut::pop(timer);
        }
    }

    /// Returns the time until the executor will be able to make progress on tasks due to internal
    /// timers popping.  The executor should be polled soon after this time (if not earlier due to
    /// IO operations becoming available).
    ///
    /// If no timers are currently programmed, returns `None`.
    pub(super) fn next_timer_pop(&mut self) -> Option<Duration> {
        let now = Instant::now();
        while let Some(timer) = self.0.peek_mut() {
            if timer.shared.lock().unwrap().waker.is_some() {
                return Some(timer.wake_at - now);
            } else {
                PeekMut::pop(timer);
            }
        }

        return None;
    }
}
