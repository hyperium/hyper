macro_rules! ready {
    ($e:expr) => {
        match $e {
            std::task::Poll::Ready(v) => v,
            std::task::Poll::Pending => return std::task::Poll::Pending,
        }
    };
}

#[cfg(all(any(feature = "client", feature = "server"), feature = "http1"))]
pub(crate) mod buf;
#[cfg(all(feature = "server", any(feature = "http1", feature = "http2")))]
pub(crate) mod date;
#[cfg(not(feature = "http2"))]
pub(crate) mod exec;
pub(crate) mod io;
pub(crate) mod task;
#[cfg(any(
    all(feature = "server", feature = "http1"),
    all(any(feature = "client", feature = "server"), feature = "http2"),
))]
pub(crate) mod time;
pub(crate) mod watch;

pub(crate) use self::task::Poll;

// group up types normally needed for `Future`
pub(crate) use std::{future::Future, pin::Pin};
