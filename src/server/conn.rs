//! Lower-level Server connection API.
//!
//! The types in thie module are to provide a lower-level API based around a
//! single connection. Accepting a connection and binding it with a service
//! are not handled at this level. This module provides the building blocks to
//! customize those things externally.
//!
//! If don't have need to manage connections yourself, consider using the
//! higher-level [Server](super) API.

use std::fmt;

use bytes::Bytes;
use futures::{Future, Poll, Stream};
use tokio_io::{AsyncRead, AsyncWrite};

use proto;
use super::{HyperService, Request, Response, Service};

/// A future binding a connection with a Service.
///
/// Polling this future will drive HTTP forward.
#[must_use = "futures do nothing unless polled"]
pub struct Connection<I, S>
where
    S: HyperService,
    S::ResponseBody: Stream<Error=::Error>,
    <S::ResponseBody as Stream>::Item: AsRef<[u8]>,
{
    pub(super) conn: proto::dispatch::Dispatcher<
        proto::dispatch::Server<S>,
        S::ResponseBody,
        I,
        <S::ResponseBody as Stream>::Item,
        proto::ServerTransaction,
    >,
}

/// Deconstructed parts of a `Connection`.
///
/// This allows taking apart a `Connection` at a later time, in order to
/// reclaim the IO object, and additional related pieces.
#[derive(Debug)]
pub struct Parts<T> {
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
    _inner: (),
}

// ===== impl Connection =====

impl<I, B, S> Connection<I, S>
where S: Service<Request = Request, Response = Response<B>, Error = ::Error> + 'static,
      I: AsyncRead + AsyncWrite + 'static,
      B: Stream<Error=::Error> + 'static,
      B::Item: AsRef<[u8]>,
{
    /// Disables keep-alive for this connection.
    pub fn disable_keep_alive(&mut self) {
        self.conn.disable_keep_alive()
    }

    /// Return the inner IO object, and additional information.
    ///
    /// This should only be called after `poll_without_shutdown` signals
    /// that the connection is "done". Otherwise, it may not have finished
    /// flushing all necessary HTTP bytes.
    pub fn into_parts(self) -> Parts<I> {
        let (io, read_buf) = self.conn.into_inner();
        Parts {
            io: io,
            read_buf: read_buf,
            _inner: (),
        }
    }

    /// Poll the connection for completion, but without calling `shutdown`
    /// on the underlying IO.
    ///
    /// This is useful to allow running a connection while doing an HTTP
    /// upgrade. Once the upgrade is completed, the connection would be "done",
    /// but it is not desired to actally shutdown the IO object. Instead you
    /// would take it back using `into_parts`.
    pub fn poll_without_shutdown(&mut self) -> Poll<(), ::Error> {
        try_ready!(self.conn.poll_without_shutdown());
        Ok(().into())
    }
}

impl<I, B, S> Future for Connection<I, S>
where S: Service<Request = Request, Response = Response<B>, Error = ::Error> + 'static,
      I: AsyncRead + AsyncWrite + 'static,
      B: Stream<Error=::Error> + 'static,
      B::Item: AsRef<[u8]>,
{
    type Item = ();
    type Error = ::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        self.conn.poll()
    }
}

impl<I, S> fmt::Debug for Connection<I, S>
where
    S: HyperService,
    S::ResponseBody: Stream<Error=::Error>,
    <S::ResponseBody as Stream>::Item: AsRef<[u8]>,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Connection")
            .finish()
    }
}

