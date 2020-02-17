use std::fmt;
use std::io;
use std::net::{SocketAddr, TcpListener as StdTcpListener};
use std::time::Duration;

use futures_util::FutureExt as _;
use tokio::net::TcpListener;
use tokio::time::Delay;

use crate::common::{task, Future, Pin, Poll};

pub use self::addr_stream::AddrStream;
use super::Accept;

/// A stream of connections from binding to an address.
#[must_use = "streams do nothing unless polled"]
pub struct AddrIncoming {
    addr: SocketAddr,
    listener: TcpListener,
    sleep_on_errors: bool,
    tcp_keepalive_timeout: Option<Duration>,
    tcp_nodelay: bool,
    timeout: Option<Delay>,
}

impl AddrIncoming {
    pub(super) fn new(addr: &SocketAddr) -> crate::Result<Self> {
        let std_listener = StdTcpListener::bind(addr).map_err(crate::Error::new_listen)?;

        AddrIncoming::from_std(std_listener)
    }

    pub(super) fn from_std(std_listener: StdTcpListener) -> crate::Result<Self> {
        let listener = TcpListener::from_std(std_listener).map_err(crate::Error::new_listen)?;
        let addr = listener.local_addr().map_err(crate::Error::new_listen)?;
        Ok(AddrIncoming {
            listener,
            addr,
            sleep_on_errors: true,
            tcp_keepalive_timeout: None,
            tcp_nodelay: false,
            timeout: None,
        })
    }

    /// Creates a new `AddrIncoming` binding to provided socket address.
    pub fn bind(addr: &SocketAddr) -> crate::Result<Self> {
        AddrIncoming::new(addr)
    }

    /// Get the local address bound to this listener.
    pub fn local_addr(&self) -> SocketAddr {
        self.addr
    }

    /// Set whether TCP keepalive messages are enabled on accepted connections.
    ///
    /// If `None` is specified, keepalive is disabled, otherwise the duration
    /// specified will be the time to remain idle before sending TCP keepalive
    /// probes.
    pub fn set_keepalive(&mut self, keepalive: Option<Duration>) -> &mut Self {
        self.tcp_keepalive_timeout = keepalive;
        self
    }

    /// Set the value of `TCP_NODELAY` option for accepted connections.
    pub fn set_nodelay(&mut self, enabled: bool) -> &mut Self {
        self.tcp_nodelay = enabled;
        self
    }

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

    fn poll_next_(&mut self, cx: &mut task::Context<'_>) -> Poll<io::Result<AddrStream>> {
        // Check if a previous timeout is active that was set by IO errors.
        if let Some(ref mut to) = self.timeout {
            match Pin::new(to).poll(cx) {
                Poll::Ready(()) => {}
                Poll::Pending => return Poll::Pending,
            }
        }
        self.timeout = None;

        let accept = self.listener.accept();
        futures_util::pin_mut!(accept);

        loop {
            match accept.poll_unpin(cx) {
                Poll::Ready(Ok((socket, addr))) => {
                    if let Some(dur) = self.tcp_keepalive_timeout {
                        if let Err(e) = socket.set_keepalive(Some(dur)) {
                            trace!("error trying to set TCP keepalive: {}", e);
                        }
                    }
                    if let Err(e) = socket.set_nodelay(self.tcp_nodelay) {
                        trace!("error trying to set TCP nodelay: {}", e);
                    }
                    return Poll::Ready(Ok(AddrStream::new(socket, addr)));
                }
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Err(e)) => {
                    // Connection errors can be ignored directly, continue by
                    // accepting the next request.
                    if is_connection_error(&e) {
                        debug!("accepted connection already errored: {}", e);
                        continue;
                    }

                    if self.sleep_on_errors {
                        error!("accept error: {}", e);

                        // Sleep 1s.
                        let mut timeout = tokio::time::delay_for(Duration::from_secs(1));

                        match Pin::new(&mut timeout).poll(cx) {
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
                        return Poll::Ready(Err(e));
                    }
                }
            }
        }
    }
}

impl Accept for AddrIncoming {
    type Conn = AddrStream;
    type Error = io::Error;

    fn poll_accept(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> Poll<Option<Result<Self::Conn, Self::Error>>> {
        let result = ready!(self.poll_next_(cx));
        Poll::Ready(Some(result))
    }
}

/// This function defines errors that are per-connection. Which basically
/// means that if we get this error from `accept()` system call it means
/// next connection might be ready to be accepted.
///
/// All other errors will incur a timeout before next `accept()` is performed.
/// The timeout is useful to handle resource exhaustion errors like ENFILE
/// and EMFILE. Otherwise, could enter into tight loop.
fn is_connection_error(e: &io::Error) -> bool {
    match e.kind() {
        io::ErrorKind::ConnectionRefused
        | io::ErrorKind::ConnectionAborted
        | io::ErrorKind::ConnectionReset => true,
        _ => false,
    }
}

impl fmt::Debug for AddrIncoming {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AddrIncoming")
            .field("addr", &self.addr)
            .field("sleep_on_errors", &self.sleep_on_errors)
            .field("tcp_keepalive_timeout", &self.tcp_keepalive_timeout)
            .field("tcp_nodelay", &self.tcp_nodelay)
            .finish()
    }
}

mod addr_stream {
    use bytes::{Buf, BufMut};
    use std::io;
    use std::net::SocketAddr;
    use tokio::io::{AsyncRead, AsyncWrite};
    use tokio::net::TcpStream;

    use crate::common::{task, Pin, Poll};

    /// A transport returned yieled by `AddrIncoming`.
    #[derive(Debug)]
    pub struct AddrStream {
        inner: TcpStream,
        pub(super) remote_addr: SocketAddr,
    }

    impl AddrStream {
        pub(super) fn new(tcp: TcpStream, addr: SocketAddr) -> AddrStream {
            AddrStream {
                inner: tcp,
                remote_addr: addr,
            }
        }

        /// Returns the remote (peer) address of this connection.
        #[inline]
        pub fn remote_addr(&self) -> SocketAddr {
            self.remote_addr
        }

        /// Consumes the AddrStream and returns the underlying IO object
        #[inline]
        pub fn into_inner(self) -> TcpStream {
            self.inner
        }

        /// Attempt to receive data on the socket, without removing that data
        /// from the queue, registering the current task for wakeup if data is
        /// not yet available.
        pub fn poll_peek(
            &mut self,
            cx: &mut task::Context<'_>,
            buf: &mut [u8],
        ) -> Poll<io::Result<usize>> {
            self.inner.poll_peek(cx, buf)
        }
    }

    impl AsyncRead for AddrStream {
        unsafe fn prepare_uninitialized_buffer(
            &self,
            buf: &mut [std::mem::MaybeUninit<u8>],
        ) -> bool {
            self.inner.prepare_uninitialized_buffer(buf)
        }

        #[inline]
        fn poll_read(
            mut self: Pin<&mut Self>,
            cx: &mut task::Context<'_>,
            buf: &mut [u8],
        ) -> Poll<io::Result<usize>> {
            Pin::new(&mut self.inner).poll_read(cx, buf)
        }

        #[inline]
        fn poll_read_buf<B: BufMut>(
            mut self: Pin<&mut Self>,
            cx: &mut task::Context<'_>,
            buf: &mut B,
        ) -> Poll<io::Result<usize>> {
            Pin::new(&mut self.inner).poll_read_buf(cx, buf)
        }
    }

    impl AsyncWrite for AddrStream {
        #[inline]
        fn poll_write(
            mut self: Pin<&mut Self>,
            cx: &mut task::Context<'_>,
            buf: &[u8],
        ) -> Poll<io::Result<usize>> {
            Pin::new(&mut self.inner).poll_write(cx, buf)
        }

        #[inline]
        fn poll_write_buf<B: Buf>(
            mut self: Pin<&mut Self>,
            cx: &mut task::Context<'_>,
            buf: &mut B,
        ) -> Poll<io::Result<usize>> {
            Pin::new(&mut self.inner).poll_write_buf(cx, buf)
        }

        #[inline]
        fn poll_flush(self: Pin<&mut Self>, _cx: &mut task::Context<'_>) -> Poll<io::Result<()>> {
            // TCP flush is a noop
            Poll::Ready(Ok(()))
        }

        #[inline]
        fn poll_shutdown(
            mut self: Pin<&mut Self>,
            cx: &mut task::Context<'_>,
        ) -> Poll<io::Result<()>> {
            Pin::new(&mut self.inner).poll_shutdown(cx)
        }
    }
}
