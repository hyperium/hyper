use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::body::{Body, HttpBody};
use crate::proto::h2::server::H2Stream;
use crate::server::conn::spawn_all::SvcTask;
use crate::service::HttpService;

/// An executor of futures.
pub trait Executor<Fut> {
    /// Place the future into the executor to be run.
    fn execute(&self, fut: Fut);
}

pub trait H2Exec<F, B: HttpBody>: Clone {
    fn execute_h2stream(&mut self, fut: H2Stream<F, B>);
}

pub trait SvcExec<I, S: HttpService<Body>, E>: Clone {
    fn execute_svc(&self, fut: SvcTask<I, S, E>);
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

impl<F, B> H2Exec<F, B> for Exec
where
    H2Stream<F, B>: Future<Output = ()> + Send + 'static,
    B: HttpBody,
{
    fn execute_h2stream(&mut self, fut: H2Stream<F, B>) {
        self.execute(fut)
    }
}

impl<I, S, E> SvcExec<I, S, E> for Exec
where
    SvcTask<I, S, E>: Future<Output = ()> + Send + 'static,
    S: HttpService<Body>,
{
    fn execute_svc(&self, fut: SvcTask<I, S, E>) {
        self.execute(fut)
    }
}

// ==== impl Executor =====

impl<E, F, B> H2Exec<F, B> for E
where
    E: Executor<H2Stream<F, B>> + Clone,
    H2Stream<F, B>: Future<Output = ()>,
    B: HttpBody,
{
    fn execute_h2stream(&mut self, fut: H2Stream<F, B>) {
        self.execute(fut)
    }
}

impl<I, S, E> SvcExec<I, S, E> for E
where
    E: Executor<SvcTask<I, S, E>> + Clone,
    SvcTask<I, S, E>: Future<Output = ()>,
    S: HttpService<Body>,
{
    fn execute_svc(&self, fut: SvcTask<I, S, E>) {
        self.execute(fut)
    }
}
