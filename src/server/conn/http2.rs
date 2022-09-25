//! HTTP-2 Connection

use std::error::Error as StdError;
use std::fmt;

use pin_project_lite::pin_project;
use tokio::io::{AsyncRead, AsyncWrite};

use crate::body::{Body, Recv};
use crate::common::exec::{ConnStreamExec, Exec};
use crate::common::{task, Future, Pin, Poll, Unpin};
use crate::{common::io::Rewind, proto, server::conn::Fallback, service::HttpService};

type Http2Server<T, B, S, E> = proto::h2::Server<Rewind<T>, S, B, E>;

pin_project! {
    /// A future binding an http2 connection with a Service.
    ///
    /// Polling this future will drive HTTP forward.
    #[must_use = "futures do nothing unless polled"]
    pub struct Connection<T, S, E = Exec>
    where
        S: HttpService<Recv>,
    {
        pub(super) conn: Option<Http2Server<T, S::ResBody, S, E>>,
        fallback: Fallback<E>,
    }
}

impl<I, S> fmt::Debug for Connection<I, S>
where
    S: HttpService<Recv>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Connection").finish()
    }
}

impl<I, B, S, E> Connection<I, S, E>
where
    S: HttpService<Recv, ResBody = B>,
    S::Error: Into<Box<dyn StdError + Send + Sync>>,
    I: AsyncRead + AsyncWrite + Unpin,
    B: Body + 'static,
    B::Error: Into<Box<dyn StdError + Send + Sync>>,
    E: ConnStreamExec<S::Future, B>,
{
    /// Start a graceful shutdown process for this connection.
    ///
    /// This `Connection` should continue to be polled until shutdown
    /// can finish.
    ///
    /// # Note
    ///
    /// This should only be called while the `Connection` future is still
    /// pending. If called after `Connection::poll` has resolved, this does
    /// nothing.
    pub fn graceful_shutdown(mut self: Pin<&mut Self>) {
        match self.conn {
            #[cfg(feature = "http2")]
            Some(ref mut h2) => {
                h2.graceful_shutdown();
            }
            None => (),
        }
    }

    /// Poll the connection for completion, but without calling `shutdown`
    /// on the underlying IO.
    ///
    /// This is useful to allow running a connection while doing an HTTP
    /// upgrade. Once the upgrade is completed, the connection would be "done",
    /// but it is not desired to actually shutdown the IO object. Instead you
    /// would take it back using `into_parts`.
    pub fn poll_without_shutdown(&mut self, cx: &mut task::Context<'_>) -> Poll<crate::Result<()>>
    where
        S: Unpin,
        S::Future: Unpin,
        B: Unpin,
    {
        Pin::new(self.conn.as_mut().unwrap())
            .poll(cx)
            .map_ok(|_| ())
    }
}

impl<I, B, S, E> Future for Connection<I, S, E>
where
    S: HttpService<Recv, ResBody = B>,
    S::Error: Into<Box<dyn StdError + Send + Sync>>,
    I: AsyncRead + AsyncWrite + Unpin + 'static,
    B: Body + 'static,
    B::Error: Into<Box<dyn StdError + Send + Sync>>,
    E: ConnStreamExec<S::Future, B>,
{
    type Output = crate::Result<()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Self::Output> {
        match ready!(Pin::new(self.conn.as_mut().unwrap()).poll(cx)) {
            Ok(done) => {
                match done {
                    proto::Dispatched::Shutdown => {}
                    #[cfg(feature = "http1")]
                    proto::Dispatched::Upgrade(pending) => {
                        // With no `Send` bound on `I`, we can't try to do
                        // upgrades here. In case a user was trying to use
                        // `Body::on_upgrade` with this API, send a special
                        // error letting them know about that.
                        pending.manual();
                    }
                };
                Poll::Ready(Ok(()))
            }
            Err(e) => {
                Poll::Ready(Err(e))
            }
        }
    }
}