//! HTTP-1 Connection

use std::error::Error as StdError;
use std::fmt;

use bytes::Bytes;
use tokio::io::{AsyncRead, AsyncWrite};
use tracing::trace;

use crate::body::{Body, Recv};
use crate::common::exec::{ConnStreamExec, Exec};
use crate::common::{task, Future, Pin, Poll, Unpin};
use crate::error::{Kind, Parse};
use crate::proto;
use crate::{server::conn::Fallback, service::HttpService};

type Http1Dispatcher<T, B, S> =
    proto::h1::Dispatcher<proto::h1::dispatch::Server<S, Recv>, B, T, proto::ServerTransaction>;

/// Deconstructed parts of a `Connection`.
///
/// This allows taking apart a `Connection` at a later time, in order to
/// reclaim the IO object, and additional related pieces.
#[derive(Debug)]
pub struct Parts<T, S> {
    /// The original IO object used in the handshake.
    pub io: T,
    /// A buffer of bytes that have been read but not processed as HTTP.
    ///
    /// If the client sent additional bytes after its last request, and
    /// this connection "ended" with an upgrade, the read buffer will contain
    /// those bytes.
    ///
    /// You will want to check for any existing bytes if you plan to continue
    /// communicating on the IO object.
    pub read_buf: Bytes,
    /// The `Service` used to serve this connection.
    pub service: S,
    _inner: (),
}

pin_project_lite::pin_project! {
    /// A future binding an http1 connection with a Service.
    ///
    /// Polling this future will drive HTTP forward.
    #[must_use = "futures do nothing unless polled"]
    // TODO: Mike(how does this docstring change)
    // #[cfg_attr(docsrs, doc(cfg(any(feature = "http1", feature = "http2"))))]
    pub struct Connection<T, S, E = Exec>
    where
        S: HttpService<Recv>,
    {
        pub(super) conn: Option<Http1Dispatcher<T, S::ResBody, S>>,
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
            Some(ref mut h1) => {
                h1.disable_keep_alive();
            }
            None => (),
        }
    }

    /// Return the inner IO object, and additional information.
    ///
    /// If the IO object has been "rewound" the io will not contain those bytes rewound.
    /// This should only be called after `poll_without_shutdown` signals
    /// that the connection is "done". Otherwise, it may not have finished
    /// flushing all necessary HTTP bytes.
    ///
    /// # Panics
    /// This method will panic if this connection is using an h2 protocol.
    pub fn into_parts(self) -> Parts<I, S> {
        self.try_into_parts()
            .unwrap_or_else(|| panic!("h2 cannot into_inner"))
    }

    /// Return the inner IO object, and additional information, if available.
    ///
    ///
    /// TODO:(mike) does this need to return none for h1 or is it expected to always be present? previously used an "unwrap"
    /// This method will return a `None` if this connection is using an h2 protocol.
    pub fn try_into_parts(self) -> Option<Parts<I, S>> {
        self.conn.map(|h1| {
            let (io, read_buf, dispatch) = h1.into_inner();
            Parts {
                io,
                read_buf,
                service: dispatch.into_service(),
                _inner: (),
            }
        })
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
        loop {
            let h1 = self.conn.as_mut().unwrap();
            match ready!(h1.poll_without_shutdown(cx)) {
                Ok(()) => return Poll::Ready(Ok(())),
                Err(e) => {
                    #[cfg(feature = "http2")]
                    match *e.kind() {
                        Kind::Parse(Parse::VersionH2) if self.fallback.to_h2() => {
                            self.upgrade_h2();
                            continue;
                        }
                        _ => (),
                    }

                    return Poll::Ready(Err(e));
                }
            }
        }
    }

    /// Prevent shutdown of the underlying IO object at the end of service the request,
    /// instead run `into_parts`. This is a convenience wrapper over `poll_without_shutdown`.
    ///
    /// # Error
    ///
    /// This errors if the underlying connection protocol is not HTTP/1.
    pub fn without_shutdown(self) -> impl Future<Output = crate::Result<Parts<I, S>>>
    where
        S: Unpin,
        S::Future: Unpin,
        B: Unpin,
    {
        // TODO(mike): "new_without_shutdown_not_h1" is not possible here
        let mut conn = Some(self);
        futures_util::future::poll_fn(move |cx| {
            ready!(conn.as_mut().unwrap().poll_without_shutdown(cx))?;
            Poll::Ready(
                conn.take()
                    .unwrap()
                    .try_into_parts()
                    .ok_or_else(crate::Error::new_without_shutdown_not_h1),
            )
        })
    }

    #[cfg(feature = "http2")]
    fn upgrade_h2(&mut self) {

        use crate::common::io::Rewind;

        trace!("Trying to upgrade connection to h2");
        let conn = self.conn.take();

        let (io, read_buf, dispatch) = conn.unwrap().into_inner();
        let mut rewind_io = Rewind::new(io);
        rewind_io.rewind(read_buf);
        let (builder, exec, timer) = match self.fallback {
            Fallback::ToHttp2(ref builder, ref exec, ref timer) => (builder, exec, timer),
            Fallback::Http1Only => unreachable!("upgrade_h2 with Fallback::Http1Only"),
        };
        let _h2 = crate::proto::h2::Server::new(
            rewind_io,
            dispatch.into_service(),
            builder,
            exec.clone(),
            timer.clone(),
        );

        debug_assert!(self.conn.is_none());
        todo!("this needs to change from &mut self to self and return return http::Conn)")
        // self.conn = Some(ProtoServer::H2 { h2 });
    }

    /// Enable this connection to support higher-level HTTP upgrades.
    ///
    /// See [the `upgrade` module](crate::upgrade) for more.
    pub fn with_upgrades(self) -> upgrades::UpgradeableConnection<I, S, E>
    where
        I: Send,
    {
        upgrades::UpgradeableConnection { inner: self }
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
        loop {
            match ready!(Pin::new(self.conn.as_mut().unwrap()).poll(cx)) {
                Ok(done) => {
                    match done {
                        proto::Dispatched::Shutdown => {}
                        proto::Dispatched::Upgrade(pending) => {
                            // With no `Send` bound on `I`, we can't try to do
                            // upgrades here. In case a user was trying to use
                            // `Body::on_upgrade` with this API, send a special
                            // error letting them know about that.
                            pending.manual();
                        }
                    };
                    return Poll::Ready(Ok(()));
                }
                Err(e) => {
                    #[cfg(feature = "http2")]
                    match *e.kind() {
                        Kind::Parse(Parse::VersionH2) if self.fallback.to_h2() => {
                            self.upgrade_h2();
                            continue;
                        }
                        _ => (),
                    }

                    return Poll::Ready(Err(e));
                }
            }
        }
    }
}















// TODO: Only available with http2?
mod upgrades {
    use crate::upgrade::Upgraded;

    use super::*;

    // A future binding a connection with a Service with Upgrade support.
    //
    // This type is unnameable outside the crate, and so basically just an
    // `impl Future`, without requiring Rust 1.26.
    #[must_use = "futures do nothing unless polled"]
    #[allow(missing_debug_implementations)]
    pub struct UpgradeableConnection<T, S, E>
    where
        S: HttpService<Recv>,
    {
        pub(super) inner: Connection<T, S, E>,
    }

    impl<I, B, S, E> UpgradeableConnection<I, S, E>
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
        pub fn graceful_shutdown(mut self: Pin<&mut Self>) {
            Pin::new(&mut self.inner).graceful_shutdown()
        }
    }

    impl<I, B, S, E> Future for UpgradeableConnection<I, S, E>
    where
        S: HttpService<Recv, ResBody = B>,
        S::Error: Into<Box<dyn StdError + Send + Sync>>,
        I: AsyncRead + AsyncWrite + Unpin + Send + 'static,
        B: Body + 'static,
        B::Error: Into<Box<dyn StdError + Send + Sync>>,
        E: ConnStreamExec<S::Future, B>,
    {
        type Output = crate::Result<()>;

        fn poll(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Self::Output> {
            loop {
                match ready!(Pin::new(self.inner.conn.as_mut().unwrap()).poll(cx)) {
                    Ok(proto::Dispatched::Shutdown) => return Poll::Ready(Ok(())),
                    Ok(proto::Dispatched::Upgrade(pending)) => {
                        // TODO: should this become an "unwrap"?
                        match self.inner.conn.take() {
                            Some(h1) => {
                                let (io, buf, _) = h1.into_inner();
                                pending.fulfill(Upgraded::new(io, buf));
                                return Poll::Ready(Ok(()));
                            }
                            _ => {
                                drop(pending);
                                unreachable!("Upgrade expects h1")
                            }
                        };
                    }
                    Err(e) => {
                        #[cfg(feature = "http2")]
                        match *e.kind() {
                            Kind::Parse(Parse::VersionH2) if self.inner.fallback.to_h2() => {
                                self.inner.upgrade_h2();
                                continue;
                            }
                            _ => (),
                        }

                        return Poll::Ready(Err(e));
                    }
                }
            }
        }
    }
}
