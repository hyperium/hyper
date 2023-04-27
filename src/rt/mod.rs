//! Runtime components
//!
//! By default, hyper includes the [tokio](https://tokio.rs) runtime.
//!
//! If the `runtime` feature is disabled, the types in this module can be used
//! to plug in other runtimes.

pub mod bounds;

use std::{
    future::Future,
    pin::Pin,
    time::{Duration, Instant},
};

/// An executor of futures.
///
/// This trait should be implemented for any future.
///
/// # Example
///
/// ```
/// # use hyper::rt::Executor;
/// # use std::future::Future;
/// #[derive(Clone)]
/// struct TokioExecutor;
///
/// impl<F> Executor<F> for TokioExecutor
/// where
///     F: Future + Send + 'static,
///     F::Output: Send + 'static,
/// {
///     fn execute(&self, future: F) {
///         tokio::spawn(future);
///     }
/// }
/// ```
pub trait Executor<Fut> {
    /// Place the future into the executor to be run.
    fn execute(&self, fut: Fut);
}

/// A timer which provides timer-like functions.
pub trait Timer {
    /// Return a future that resolves in `duration` time.
    fn sleep(&self, duration: Duration) -> Pin<Box<dyn Sleep>>;

    /// Return a future that resolves at `deadline`.
    fn sleep_until(&self, deadline: Instant) -> Pin<Box<dyn Sleep>>;

    /// Reset a future to resolve at `new_deadline` instead.
    fn reset(&self, sleep: &mut Pin<Box<dyn Sleep>>, new_deadline: Instant) {
        *sleep = self.sleep_until(new_deadline);
    }
}

/// A future returned by a `Timer`.
pub trait Sleep: Send + Sync + Future<Output = ()> {}
