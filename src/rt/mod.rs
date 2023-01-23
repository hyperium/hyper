//! Runtime components
//!
//! By default, hyper includes the [tokio](https://tokio.rs) runtime.
//!
//! If the `runtime` feature is disabled, the types in this module can be used
//! to plug in other runtimes.

pub mod bounds;
pub mod timer;

pub use timer::*;

/// An executor of futures.
///
/// This trait should be implemented for any future.
///
/// # Example
///
/// ```
/// # use hyper::rt::Executor;
/// # use std::future::Future;
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
