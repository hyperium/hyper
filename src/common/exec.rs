use std::fmt;
use std::sync::Arc;

use futures::future::{Executor, Future};

use body::Payload;
use proto::h2::server::H2Stream;
use server::conn::spawn_all::{NewSvcTask, Watcher};
use service::Service;

pub trait H2Exec<F, B: Payload>: Clone {
    fn execute_h2stream(&self, fut: H2Stream<F, B>) -> ::Result<()>;
}

pub trait NewSvcExec<I, N, S: Service, E, W: Watcher<I, S, E>>: Clone {
    fn execute_new_svc(&self, fut: NewSvcTask<I, N, S, E, W>) -> ::Result<()>;
}

// Either the user provides an executor for background tasks, or we use
// `tokio::spawn`.
#[derive(Clone)]
pub enum Exec {
    Default,
    Executor(Arc<dyn Executor<Box<dyn Future<Item=(), Error=()> + Send>> + Send + Sync>),
}

// ===== impl Exec =====

impl Exec {
    pub(crate) fn execute<F>(&self, fut: F) -> ::Result<()>
    where
        F: Future<Item=(), Error=()> + Send + 'static,
    {
        match *self {
            Exec::Default => {
                #[cfg(feature = "runtime")]
                {
                    use std::error::Error as StdError;
                    use ::tokio_executor::Executor;

                    struct TokioSpawnError;

                    impl fmt::Debug for TokioSpawnError {
                        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                            fmt::Debug::fmt("tokio::spawn failed (is a tokio runtime running this future?)", f)
                        }
                    }

                    impl fmt::Display for TokioSpawnError {
                        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                            fmt::Display::fmt("tokio::spawn failed (is a tokio runtime running this future?)", f)
                        }
                    }

                    impl StdError for TokioSpawnError {
                        fn description(&self) -> &str {
                            "tokio::spawn failed"
                        }
                    }

                    ::tokio_executor::DefaultExecutor::current()
                        .spawn(Box::new(fut))
                        .map_err(|err| {
                            warn!("executor error: {:?}", err);
                            ::Error::new_execute(TokioSpawnError)
                        })
                }
                #[cfg(not(feature = "runtime"))]
                {
                    // If no runtime, we need an executor!
                    panic!("executor must be set")
                }
            },
            Exec::Executor(ref e) => {
                e.execute(Box::new(fut))
                    .map_err(|err| {
                        warn!("executor error: {:?}", err.kind());
                        ::Error::new_execute("custom executor failed")
                    })
            },
        }
    }
}

impl fmt::Debug for Exec {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Exec")
            .finish()
    }
}


impl<F, B> H2Exec<F, B> for Exec
where
    H2Stream<F, B>: Future<Item=(), Error=()> + Send + 'static,
    B: Payload,
{
    fn execute_h2stream(&self, fut: H2Stream<F, B>) -> ::Result<()> {
        self.execute(fut)
    }
}

impl<I, N, S, E, W> NewSvcExec<I, N, S, E, W> for Exec
where
    NewSvcTask<I, N, S, E, W>: Future<Item=(), Error=()> + Send + 'static,
    S: Service,
    W: Watcher<I, S, E>,
{
    fn execute_new_svc(&self, fut: NewSvcTask<I, N, S, E, W>) -> ::Result<()> {
        self.execute(fut)
    }
}

// ==== impl Executor =====

impl<E, F, B> H2Exec<F, B> for E
where
    E: Executor<H2Stream<F, B>> + Clone,
    H2Stream<F, B>: Future<Item=(), Error=()>,
    B: Payload,
{
    fn execute_h2stream(&self, fut: H2Stream<F, B>) -> ::Result<()> {
        self.execute(fut)
            .map_err(|err| {
                warn!("executor error: {:?}", err.kind());
                ::Error::new_execute("custom executor failed")
            })
    }
}

impl<I, N, S, E, W> NewSvcExec<I, N, S, E, W> for E
where
    E: Executor<NewSvcTask<I, N, S, E, W>> + Clone,
    NewSvcTask<I, N, S, E, W>: Future<Item=(), Error=()>,
    S: Service,
    W: Watcher<I, S, E>,
{
    fn execute_new_svc(&self, fut: NewSvcTask<I, N, S, E, W>) -> ::Result<()> {
        self.execute(fut)
            .map_err(|err| {
                warn!("executor error: {:?}", err.kind());
                ::Error::new_execute("custom executor failed")
            })
    }
}

// ===== StdError impls =====

