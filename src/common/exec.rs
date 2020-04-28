use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

/// An executor of futures.
pub trait Executor<Fut> {
    /// Place the future into the executor to be run.
    fn execute(&self, fut: Fut);
}

pub trait Task: sealed::Sealed + Future<Output = ()> + Send + 'static {}

pub(crate) mod sealed {
    pub trait Sealed {}
}

pub type BoxSendFuture = Pin<Box<dyn Future<Output = ()> + Send>>;

// Either the user provides an executor for background tasks, or we use
// `tokio::spawn`.
#[derive(Clone)]
pub enum Exec {
    Default,
    Executor(Arc<dyn Executor<BoxSendFuture> + Send + Sync>),
}

// ===== impl Exec =====

impl Exec {
    pub(crate) fn execute<F>(&self, fut: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        match *self {
            Exec::Default => {
                #[cfg(feature = "tcp")]
                {
                    tokio::task::spawn(fut);
                }
                #[cfg(not(feature = "tcp"))]
                {
                    // If no runtime, we need an executor!
                    panic!("executor must be set")
                }
            }
            Exec::Executor(ref e) => {
                e.execute(Box::pin(fut));
            }
        }
    }
}

impl fmt::Debug for Exec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Exec").finish()
    }
}

// ==== impl Executor =====

impl<T> Executor<T> for Exec
where
    T: Task,
{
    fn execute(&self, fut: T) {
        self.execute(fut)
    }
}
