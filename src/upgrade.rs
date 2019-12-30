//! HTTP Upgrades
//!
//! See [this example][example] showing how upgrades work with both
//! Clients and Servers.
//!
//! [example]: https://github.com/hyperium/hyper/blob/master/examples/upgrades.rs

use std::any::TypeId;
use std::error::Error as StdError;
use std::fmt;
use std::io;
use std::marker::Unpin;

use bytes::{Buf, Bytes};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::oneshot;

use crate::common::io::Rewind;
use crate::common::{task, Future, Pin, Poll};

/// An upgraded HTTP connection.
///
/// This type holds a trait object internally of the original IO that
/// was used to speak HTTP before the upgrade. It can be used directly
/// as a `Read` or `Write` for convenience.
///
/// Alternatively, if the exact type is known, this can be deconstructed
/// into its parts.
pub struct Upgraded {
    io: Rewind<Box<dyn Io + Send>>,
}

/// A future for a possible HTTP upgrade.
///
/// If no upgrade was available, or it doesn't succeed, yields an `Error`.
pub struct OnUpgrade {
    rx: Option<oneshot::Receiver<crate::Result<Upgraded>>>,
}

/// The deconstructed parts of an [`Upgraded`](Upgraded) type.
///
/// Includes the original IO type, and a read buffer of bytes that the
/// HTTP state machine may have already read before completing an upgrade.
#[derive(Debug)]
pub struct Parts<T> {
    /// The original IO object used before the upgrade.
    pub io: T,
    /// A buffer of bytes that have been read but not processed as HTTP.
    ///
    /// For instance, if the `Connection` is used for an HTTP upgrade request,
    /// it is possible the server sent back the first bytes of the new protocol
    /// along with the response upgrade.
    ///
    /// You will want to check for any existing bytes if you plan to continue
    /// communicating on the IO object.
    pub read_buf: Bytes,
    _inner: (),
}

pub(crate) struct Pending {
    tx: oneshot::Sender<crate::Result<Upgraded>>,
}

/// Error cause returned when an upgrade was expected but canceled
/// for whatever reason.
///
/// This likely means the actual `Conn` future wasn't polled and upgraded.
#[derive(Debug)]
struct UpgradeExpected(());

pub(crate) fn pending() -> (Pending, OnUpgrade) {
    let (tx, rx) = oneshot::channel();
    (Pending { tx }, OnUpgrade { rx: Some(rx) })
}

// ===== impl Upgraded =====

impl Upgraded {
    pub(crate) fn new<T>(io: T, read_buf: Bytes) -> Self
    where
        T: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        Upgraded {
            io: Rewind::new_buffered(Box::new(ForwardsWriteBuf(io)), read_buf),
        }
    }

    /// Tries to downcast the internal trait object to the type passed.
    ///
    /// On success, returns the downcasted parts. On error, returns the
    /// `Upgraded` back.
    pub fn downcast<T: AsyncRead + AsyncWrite + Unpin + 'static>(self) -> Result<Parts<T>, Self> {
        let (io, buf) = self.io.into_inner();
        match io.__hyper_downcast::<ForwardsWriteBuf<T>>() {
            Ok(t) => Ok(Parts {
                io: t.0,
                read_buf: buf,
                _inner: (),
            }),
            Err(io) => Err(Upgraded {
                io: Rewind::new_buffered(io, buf),
            }),
        }
    }
}

impl AsyncRead for Upgraded {
    unsafe fn prepare_uninitialized_buffer(&self, buf: &mut [std::mem::MaybeUninit<u8>]) -> bool {
        self.io.prepare_uninitialized_buffer(buf)
    }

    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.io).poll_read(cx, buf)
    }
}

impl AsyncWrite for Upgraded {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.io).poll_write(cx, buf)
    }

    fn poll_write_buf<B: Buf>(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
        buf: &mut B,
    ) -> Poll<io::Result<usize>> {
        Pin::new(self.io.get_mut()).poll_write_dyn_buf(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.io).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.io).poll_shutdown(cx)
    }
}

impl fmt::Debug for Upgraded {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Upgraded").finish()
    }
}

// ===== impl OnUpgrade =====

impl OnUpgrade {
    pub(crate) fn none() -> Self {
        OnUpgrade { rx: None }
    }

    pub(crate) fn is_none(&self) -> bool {
        self.rx.is_none()
    }
}

impl Future for OnUpgrade {
    type Output = Result<Upgraded, crate::Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Self::Output> {
        match self.rx {
            Some(ref mut rx) => Pin::new(rx).poll(cx).map(|res| match res {
                Ok(Ok(upgraded)) => Ok(upgraded),
                Ok(Err(err)) => Err(err),
                Err(_oneshot_canceled) => {
                    Err(crate::Error::new_canceled().with(UpgradeExpected(())))
                }
            }),
            None => Poll::Ready(Err(crate::Error::new_user_no_upgrade())),
        }
    }
}

impl fmt::Debug for OnUpgrade {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OnUpgrade").finish()
    }
}

// ===== impl Pending =====

impl Pending {
    pub(crate) fn fulfill(self, upgraded: Upgraded) {
        trace!("pending upgrade fulfill");
        let _ = self.tx.send(Ok(upgraded));
    }

    /// Don't fulfill the pending Upgrade, but instead signal that
    /// upgrades are handled manually.
    pub(crate) fn manual(self) {
        trace!("pending upgrade handled manually");
        let _ = self.tx.send(Err(crate::Error::new_user_manual_upgrade()));
    }
}

// ===== impl UpgradeExpected =====

impl fmt::Display for UpgradeExpected {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "upgrade expected but not completed")
    }
}

impl StdError for UpgradeExpected {}

// ===== impl Io =====

struct ForwardsWriteBuf<T>(T);

pub(crate) trait Io: AsyncRead + AsyncWrite + Unpin + 'static {
    fn poll_write_dyn_buf(
        &mut self,
        cx: &mut task::Context<'_>,
        buf: &mut dyn Buf,
    ) -> Poll<io::Result<usize>>;

    fn __hyper_type_id(&self) -> TypeId {
        TypeId::of::<Self>()
    }
}

impl dyn Io + Send {
    fn __hyper_is<T: Io>(&self) -> bool {
        let t = TypeId::of::<T>();
        self.__hyper_type_id() == t
    }

    fn __hyper_downcast<T: Io>(self: Box<Self>) -> Result<Box<T>, Box<Self>> {
        if self.__hyper_is::<T>() {
            // Taken from `std::error::Error::downcast()`.
            unsafe {
                let raw: *mut dyn Io = Box::into_raw(self);
                Ok(Box::from_raw(raw as *mut T))
            }
        } else {
            Err(self)
        }
    }
}

impl<T: AsyncRead + Unpin> AsyncRead for ForwardsWriteBuf<T> {
    unsafe fn prepare_uninitialized_buffer(&self, buf: &mut [std::mem::MaybeUninit<u8>]) -> bool {
        self.0.prepare_uninitialized_buffer(buf)
    }

    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.0).poll_read(cx, buf)
    }
}

impl<T: AsyncWrite + Unpin> AsyncWrite for ForwardsWriteBuf<T> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.0).poll_write(cx, buf)
    }

    fn poll_write_buf<B: Buf>(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
        buf: &mut B,
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.0).poll_write_buf(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.0).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.0).poll_shutdown(cx)
    }
}

impl<T: AsyncRead + AsyncWrite + Unpin + 'static> Io for ForwardsWriteBuf<T> {
    fn poll_write_dyn_buf(
        &mut self,
        cx: &mut task::Context<'_>,
        mut buf: &mut dyn Buf,
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.0).poll_write_buf(cx, &mut buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncWriteExt;

    #[test]
    fn upgraded_downcast() {
        let upgraded = Upgraded::new(Mock, Bytes::new());

        let upgraded = upgraded.downcast::<std::io::Cursor<Vec<u8>>>().unwrap_err();

        upgraded.downcast::<Mock>().unwrap();
    }

    #[tokio::test]
    async fn upgraded_forwards_write_buf() {
        // sanity check that the underlying IO implements write_buf
        Mock.write_buf(&mut "hello".as_bytes()).await.unwrap();

        let mut upgraded = Upgraded::new(Mock, Bytes::new());
        upgraded.write_buf(&mut "hello".as_bytes()).await.unwrap();
    }

    // TODO: replace with tokio_test::io when it can test write_buf
    struct Mock;

    impl AsyncRead for Mock {
        fn poll_read(
            self: Pin<&mut Self>,
            _cx: &mut task::Context<'_>,
            _buf: &mut [u8],
        ) -> Poll<io::Result<usize>> {
            unreachable!("Mock::poll_read")
        }
    }

    impl AsyncWrite for Mock {
        fn poll_write(
            self: Pin<&mut Self>,
            _cx: &mut task::Context<'_>,
            _buf: &[u8],
        ) -> Poll<io::Result<usize>> {
            panic!("poll_write shouldn't be called");
        }

        fn poll_write_buf<B: Buf>(
            self: Pin<&mut Self>,
            _cx: &mut task::Context<'_>,
            buf: &mut B,
        ) -> Poll<io::Result<usize>> {
            let n = buf.remaining();
            buf.advance(n);
            Poll::Ready(Ok(n))
        }

        fn poll_flush(self: Pin<&mut Self>, _cx: &mut task::Context<'_>) -> Poll<io::Result<()>> {
            unreachable!("Mock::poll_flush")
        }

        fn poll_shutdown(
            self: Pin<&mut Self>,
            _cx: &mut task::Context<'_>,
        ) -> Poll<io::Result<()>> {
            unreachable!("Mock::poll_shutdown")
        }
    }
}
