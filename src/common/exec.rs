use std::fmt;
use std::sync::Arc;

use futures::future::{Executor, Future};

/// Either the user provides an executor for background tasks, or we use
/// `tokio::spawn`.
#[derive(Clone)]
pub(crate) enum Exec {
    Default,
    Executor(Arc<Executor<Box<Future<Item=(), Error=()> + Send>> + Send + Sync>),
}


impl Exec {
    pub(crate) fn execute<F>(&self, fut: F) -> ::Result<()>
    where
        F: Future<Item=(), Error=()> + Send + 'static,
    {
        match *self {
            Exec::Default => {
                #[cfg(feature = "runtime")]
                {
                    use ::tokio_executor::Executor;
                    ::tokio_executor::DefaultExecutor::current()
                        .spawn(Box::new(fut))
                        .map_err(|err| {
                            warn!("executor error: {:?}", err);
                            ::Error::new_execute()
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
                        ::Error::new_execute()
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
