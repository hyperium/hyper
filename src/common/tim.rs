use std::{
    sync::Arc,
    time::{Duration, Instant}
};

use crate::rt::{Interval, Sleep, Timer};

/// A user-provided timer to time background tasks.
pub(crate) type Tim = Option<Arc<dyn Timer + Send + Sync>>;

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

impl Timer for Tim {
    fn sleep(&self, duration: Duration) -> Box<dyn Sleep + Unpin> {
        match *self {
            None => {
                panic!("You must supply a timer.")
            }
            Some(ref t) => t.sleep(duration),
        }
    }
    fn sleep_until(&self, deadline: Instant) -> Box<dyn Sleep + Unpin> {
        match *self {
            None => {
                panic!("You must supply a timer.")
            }
            Some(ref t) => t.sleep_until(deadline),
        }
    }

    fn interval(&self, period: Duration) -> Box<dyn Interval> {
        match *self {
            None => {
                panic!("You must supply a timer.")
            }
            Some(ref t) => t.interval(period),
        }
    }
} 
