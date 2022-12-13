#![allow(dead_code)]
//! Various runtimes for hyper
use std::{
    pin::Pin,
    task::{Context, Poll},
    time::{Duration, Instant},
};

use futures_util::Future;
use hyper::rt::{Sleep, Timer};
use pin_project_lite::pin_project;

#[derive(Clone)]
/// An Executor that uses the tokio runtime.
pub struct TokioExecutor;

impl<F> hyper::rt::Executor<F> for TokioExecutor
where
    F: std::future::Future + Send + 'static,
    F::Output: Send + 'static,
{
    fn execute(&self, fut: F) {
        tokio::task::spawn(fut);
    }
}

/// A Timer that uses the tokio runtime.

#[derive(Clone, Debug)]
pub struct TokioTimer;

impl Timer for TokioTimer {
    fn sleep(&self, duration: Duration) -> Pin<Box<dyn Sleep>> {
        Box::pin(TokioSleep {
            inner: tokio::time::sleep(duration),
        })
    }

    fn sleep_until(&self, deadline: Instant) -> Pin<Box<dyn Sleep>> {
        Box::pin(TokioSleep {
            inner: tokio::time::sleep_until(deadline.into()),
        })
    }
}

struct TokioTimeout<T> {
    inner: Pin<Box<tokio::time::Timeout<T>>>,
}

impl<T> Future for TokioTimeout<T>
where
    T: Future,
{
    type Output = Result<T::Output, tokio::time::error::Elapsed>;

    fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        self.inner.as_mut().poll(context)
    }
}

// Use TokioSleep to get tokio::time::Sleep to implement Unpin.
// see https://docs.rs/tokio/latest/tokio/time/struct.Sleep.html
pin_project! {
    pub(crate) struct TokioSleep {
        #[pin]
        pub(crate) inner: tokio::time::Sleep,
    }
}

impl Future for TokioSleep {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.project().inner.poll(cx)
    }
}

// Use HasSleep to get tokio::time::Sleep to implement Unpin.
// see https://docs.rs/tokio/latest/tokio/time/struct.Sleep.html

impl Sleep for TokioSleep {}
