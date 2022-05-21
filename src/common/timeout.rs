use std::future::Future;
use std::pin::Pin;
use std::task::Context;
use std::time::Duration;
use tokio::time::{sleep, Instant, Sleep};

/// Impl Timeout whit tokio sleep
pub(crate) struct Timeout {
    timeout_fut: Option<Pin<Box<Sleep>>>,
    timeout: Duration,
}

impl Timeout {
    pub(crate) fn new(timeout: Duration) -> Self {
        Self {
            timeout_fut: None,
            timeout,
        }
    }

    pub(crate) fn reset(&mut self, timeout: Duration) {
        if let Some(timeout_fut) = self.timeout_fut.as_mut() {
            let next_wake = Instant::now() + timeout;
            timeout_fut.as_mut().reset(next_wake);
        }
    }

    pub(crate) fn flush_time(&mut self) {
        self.reset(self.timeout);
    }

    pub(crate) fn poll_elapsed(&mut self, cx: &mut Context<'_>) -> bool {
        if let Some(timeout_fut) = self.timeout_fut.as_mut() {
            timeout_fut.as_mut().poll(cx).is_ready()
        } else {
            let mut timeout_fut = Box::pin(sleep(self.timeout));
            let is_ready = timeout_fut.as_mut().poll(cx).is_ready();
            self.timeout_fut = Some(timeout_fut);
            is_ready
        }
    }
}
