use std::{
    fmt,
    sync::Arc,
    time::{Duration, Instant},
};

use crate::rt::{Sleep, Timer};

/// A user-provided timer to time background tasks.
//pub(crate) type Time = Option<Arc<dyn Timer + Send + Sync>>;

#[derive(Clone)]
pub(crate) enum Time {
    Timer(Arc<dyn Timer + Send + Sync>),
    Empty,
}

impl fmt::Debug for Time {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Time").finish()
    }
}

/*
pub(crate) fn timeout<F>(tim: Tim, future: F, duration: Duration) -> HyperTimeout<F> {
    HyperTimeout { sleep: tim.sleep(duration), future: future }
}

pin_project_lite::pin_project! {
    pub(crate) struct HyperTimeout<F> {
        sleep: Box<dyn Sleep>,
        #[pin]
        future: F
    }
}

pub(crate) struct Timeout;

impl<F> Future for HyperTimeout<F> where F: Future {

    type Output = Result<F::Output, Timeout>;

    fn poll(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output>{
        let mut this = self.project();
        if let Poll::Ready(v) = this.future.poll(ctx) {
            return Poll::Ready(Ok(v));
        }

        if let Poll::Ready(_) = Pin::new(&mut this.sleep).poll(ctx) {
            return Poll::Ready(Err(Timeout));
        }

        return Poll::Pending;
    }
}
*/

use crate::common::time::Time::Timer as SomeTimer;

impl Timer for Time {
    fn sleep(&self, duration: Duration) -> Box<dyn Sleep + Unpin> {
        match *self {
            Empty => {
                panic!("You must supply a timer.")
            }
            SomeTimer(ref t) => t.sleep(duration),
        }
    }
    fn sleep_until(&self, deadline: Instant) -> Box<dyn Sleep + Unpin> {
        match *self {
            Empty => {
                panic!("You must supply a timer.")
            }
            SomeTimer(ref t) => t.sleep_until(deadline),
        }
    }
}
