//! The `Accept` trait and supporting types.
//!
//! This module contains:
//!
//! - The [`Accept`](Accept) trait used to asynchronously accept incoming
//!   connections.
//! - Utilities like `poll_fn` to ease creating a custom `Accept`.

use std::fmt;
use std::time::Duration;
use std::future::Future;

#[cfg(feature = "stream")]
use futures_core::Stream;
#[cfg(feature = "stream")]
use pin_project_lite::pin_project;

use tokio::time::Sleep;

use crate::common::{
    task::{self, Poll},
    Pin,
};

/// Asynchronously accept incoming connections.
pub trait Accept {
    /// The connection type that can be accepted.
    type Conn;
    /// The error type that can occur when accepting a connection.
    type Error;

    /// Poll to accept the next connection.
    fn poll_accept(
        self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> Poll<Option<Result<Self::Conn, Self::Error>>>;
}

/// Structure that implements the Accept trait and is able to sleep on errors when configured.
pub struct AcceptWithSleep<A: Accept> {
    accept: A,
    sleep_on_errors: bool,
    timeout: Option<Pin<Box<Sleep>>>,
}

impl<A: Accept> fmt::Debug for AcceptWithSleep<A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AcceptWithSleep")
            .field("sleep_on_errors", &self.sleep_on_errors)
            .finish()
    }
}

impl<A: Accept> AcceptWithSleep<A> {
    /// Set whether to sleep on accept errors.
    ///
    /// A possible scenario is that the process has hit the max open files
    /// allowed, and so trying to accept a new connection will fail with
    /// `EMFILE`. In some cases, it's preferable to just wait for some time, if
    /// the application will likely close some files (or connections), and try
    /// to accept the connection again. If this option is `true`, the error
    /// will be logged at the `error` level, since it is still a big deal,
    /// and then the listener will sleep for 1 second.
    ///
    /// In other cases, hitting the max open files should be treat similarly
    /// to being out-of-memory, and simply error (and shutdown). Setting
    /// this option to `false` will allow that.
    ///
    /// Default is `true`.
    pub fn set_sleep_on_errors(&mut self, val: bool) {
        self.sleep_on_errors = val;
    }
}

impl<A: Accept + Unpin> Accept for AcceptWithSleep<A>
{
    type Conn = A::Conn;
    type Error = A::Error;

    fn poll_accept(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> Poll<Option<Result<Self::Conn, Self::Error>>> {
        // Check if a previous timeout is active that was set by IO errors.
        if let Some(ref mut to) = self.timeout {
            ready!(Pin::new(to).poll(cx));
        }
        self.timeout = None;

        loop {
            match ready!(Pin::new(&mut self.accept).poll_accept(cx)) {
                None => return Poll::Ready(None),
                Some(Ok(item)) => return Poll::Ready(Some(Ok(item))),
                Some(Err(e)) => if self.sleep_on_errors {
                    // Sleep 1s.
                    let mut timeout = Box::pin(tokio::time::sleep(Duration::from_secs(1)));
                    match timeout.as_mut().poll(cx) {
                        Poll::Ready(()) => {
                            // Wow, it's been a second already? Ok then...
                            continue;
                        }
                        Poll::Pending => {
                            self.timeout = Some(timeout);
                            return Poll::Pending;
                        }
                    }
                } else {
                    return Poll::Ready(Some(Err(e)));
                }
            }
        }
    }
}

/// Create an `Accept` with a polling function.
///
/// # Example
///
/// ```
/// use std::task::Poll;
/// use hyper::server::{accept, Server};
///
/// # let mock_conn = ();
/// // If we created some mocked connection...
/// let mut conn = Some(mock_conn);
///
/// // And accept just the mocked conn once...
/// let once = accept::poll_fn(move |cx| {
///     Poll::Ready(conn.take().map(Ok::<_, ()>))
/// });
///
/// let builder = Server::builder(once);
/// ```
pub fn poll_fn<F, IO, E>(func: F) -> AcceptWithSleep<impl Accept<Conn = IO, Error = E>>
where
    F: FnMut(&mut task::Context<'_>) -> Poll<Option<Result<IO, E>>>,
{
    struct PollFn<F>(F);

    // The closure `F` is never pinned
    impl<F> Unpin for PollFn<F> {}

    impl<F, IO, E> Accept for PollFn<F>
    where
        F: FnMut(&mut task::Context<'_>) -> Poll<Option<Result<IO, E>>>
    {
        type Conn = IO;
        type Error = E;
        fn poll_accept(
            self: Pin<&mut Self>,
            cx: &mut task::Context<'_>,
        ) -> Poll<Option<Result<Self::Conn, Self::Error>>> {
            (self.get_mut().0)(cx)
        }
    }

    AcceptWithSleep { accept: PollFn(func), sleep_on_errors: true, timeout: None }
}

/// Adapt a `Stream` of incoming connections into an `Accept`.
///
/// # Optional
///
/// This function requires enabling the `stream` feature in your
/// `Cargo.toml`.
#[cfg(feature = "stream")]
pub fn from_stream<S, IO, E>(stream: S) -> AcceptWithSleep<impl Accept<Conn = IO, Error = E>>
where
    S: Stream<Item = Result<IO, E>>
{
    pin_project! {
        struct FromStream<S> {
            #[pin]
            stream: S,
        }
    }

    impl<S, IO, E> Accept for FromStream<S>
    where
        S: Stream<Item = Result<IO, E>>,
    {
        type Conn = IO;
        type Error = E;
        fn poll_accept(
            self: Pin<&mut Self>,
            cx: &mut task::Context<'_>,
        ) -> Poll<Option<Result<Self::Conn, Self::Error>>> {
            self.project().stream.poll_next(cx)
        }
    }

    AcceptWithSleep { accept: FromStream { stream }, sleep_on_errors: true, timeout: None }
}
