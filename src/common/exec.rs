use std::fmt;
use std::sync::Arc;

use futures::future::{Executor, Future};
use tokio_executor::spawn;

/// Either the user provides an executor for background tasks, or we use
/// `tokio::spawn`.
#[derive(Clone)]
pub(crate) enum Exec {
    Default,
    Executor(Arc<Executor<Box<Future<Item=(), Error=()> + Send>> + Send + Sync>),
}


impl Exec {
    pub(crate) fn execute<F>(&self, fut: F)
    where
        F: Future<Item=(), Error=()> + Send + 'static,
    {
        match *self {
            Exec::Default => spawn(fut),
            Exec::Executor(ref e) => {
                let _ = e.execute(Box::new(fut))
                    .map_err(|err| {
                        panic!("executor error: {:?}", err.kind());
                    });
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
