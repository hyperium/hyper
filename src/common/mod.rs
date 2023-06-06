macro_rules! ready {
    ($e:expr) => {
        match $e {
            std::task::Poll::Ready(v) => v,
            std::task::Poll::Pending => return std::task::Poll::Pending,
        }
    };
}

pub(crate) mod buf;
#[cfg(all(feature = "server", any(feature = "http1", feature = "http2")))]
pub(crate) mod date;
#[cfg(not(feature = "http2"))]
pub(crate) mod exec;
pub(crate) mod io;
mod never;
pub(crate) mod task;
#[cfg(any(feature = "http1", feature = "http2", feature = "server"))]
pub(crate) mod time;
pub(crate) mod watch;

#[cfg(any(feature = "http1", feature = "http2"))]
pub(crate) use self::never::Never;
pub(crate) use self::task::Poll;

// group up types normally needed for `Future`
cfg_proto! {
    pub(crate) use std::marker::Unpin;
}
pub(crate) use std::{future::Future, pin::Pin};
