macro_rules! ready {
    ($e:expr) => {
        match $e {
            std::task::Poll::Ready(v) => v,
            std::task::Poll::Pending => return std::task::Poll::Pending,
        }
    };
}

pub(crate) mod buf;
pub(crate) mod drain;
pub(crate) mod exec;
pub(crate) mod io;
mod lazy;
mod never;
pub(crate) mod task;
pub(crate) mod watch;

pub use self::exec::Executor;
pub(crate) use self::exec::{BoxSendFuture, Exec};
pub(crate) use self::lazy::{lazy, Started as Lazy};
pub use self::never::Never;
pub(crate) use self::task::Poll;

// group up types normally needed for `Future`
pub(crate) use std::{future::Future, marker::Unpin, pin::Pin};
