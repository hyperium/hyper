use std::fmt;
use std::io;
use std::net::{SocketAddr, TcpListener as StdTcpListener};
use std::time::Duration;

use futures::{Async, Future, Poll, Stream};
use futures_timer::Delay;
use tokio_tcp::TcpListener;
use tokio_reactor::Handle;

use self::addr_stream::AddrStream;

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
    pub(super) fn new(addr: &SocketAddr, handle: Option<&Handle>) -> ::Result<AddrIncoming> {
        let listener = if let Some(handle) = handle {
            let std_listener = StdTcpListener::bind(addr)
                .map_err(::Error::new_listen)?;
            TcpListener::from_std(std_listener, handle)
                .map_err(::Error::new_listen)?
        } else {
            TcpListener::bind(addr).map_err(::Error::new_listen)?
        };

        let addr = listener.local_addr().map_err(::Error::new_listen)?;

        Ok(AddrIncoming {
            addr: addr,
            listener: listener,
            sleep_on_errors: true,
            tcp_keepalive_timeout: None,
            tcp_nodelay: false,
            timeout: None,
        })
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
}

impl Stream for AddrIncoming {
    // currently unnameable...
    type Item = AddrStream;
    type Error = ::std::io::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        // Check if a previous timeout is active that was set by IO errors.
        if let Some(ref mut to) = self.timeout {
            match to.poll().expect("timeout never fails") {
                Async::Ready(_) => {}
                Async::NotReady => return Ok(Async::NotReady),
            }
        }
        self.timeout = None;
        loop {
            match self.listener.poll_accept() {
                Ok(Async::Ready((socket, addr))) => {
                    if let Some(dur) = self.tcp_keepalive_timeout {
                        if let Err(e) = socket.set_keepalive(Some(dur)) {
                            trace!("error trying to set TCP keepalive: {}", e);
                        }
                    }
                    if let Err(e) = socket.set_nodelay(self.tcp_nodelay) {
                        trace!("error trying to set TCP nodelay: {}", e);
                    }
                    return Ok(Async::Ready(Some(AddrStream::new(socket, addr))));
                },
                Ok(Async::NotReady) => return Ok(Async::NotReady),
                Err(ref e) if self.sleep_on_errors => {
                    // Connection errors can be ignored directly, continue by
                    // accepting the next request.
                    if is_connection_error(e) {
                        debug!("accepted connection already errored: {}", e);
                        continue;
                    }
                    // Sleep 1s.
                    let delay = Duration::from_secs(1);
                    error!("accept error: {}", e);
                    let mut timeout = Delay::new(delay);
                    let result = timeout.poll()
                        .expect("timeout never fails");
                    match result {
                        Async::Ready(()) => continue,
                        Async::NotReady => {
                            self.timeout = Some(timeout);
                            return Ok(Async::NotReady);
                        }
                    }
                },
                Err(e) => return Err(e),
            }
        }
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
    e.kind() == io::ErrorKind::ConnectionRefused ||
    e.kind() == io::ErrorKind::ConnectionAborted ||
    e.kind() == io::ErrorKind::ConnectionReset
}

impl fmt::Debug for AddrIncoming {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("AddrIncoming")
            .field("addr", &self.addr)
            .field("sleep_on_errors", &self.sleep_on_errors)
            .field("tcp_keepalive_timeout", &self.tcp_keepalive_timeout)
            .field("tcp_nodelay", &self.tcp_nodelay)
            .finish()
    }
}

mod addr_stream {
    use std::io::{self, Read, Write};
    use std::net::SocketAddr;
    use bytes::{Buf, BufMut};
    use futures::Poll;
    use tokio_tcp::TcpStream;
    use tokio_io::{AsyncRead, AsyncWrite};


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
    }

    impl Read for AddrStream {
        #[inline]
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            self.inner.read(buf)
        }
    }

    impl Write for AddrStream {
        #[inline]
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.inner.write(buf)
        }

        #[inline]
        fn flush(&mut self ) -> io::Result<()> {
            self.inner.flush()
        }
    }

    impl AsyncRead for AddrStream {
        #[inline]
        unsafe fn prepare_uninitialized_buffer(&self, buf: &mut [u8]) -> bool {
            self.inner.prepare_uninitialized_buffer(buf)
        }

        #[inline]
        fn read_buf<B: BufMut>(&mut self, buf: &mut B) -> Poll<usize, io::Error> {
            self.inner.read_buf(buf)
        }
    }

    impl AsyncWrite for AddrStream {
        #[inline]
        fn shutdown(&mut self) -> Poll<(), io::Error> {
            AsyncWrite::shutdown(&mut self.inner)
        }

        #[inline]
        fn write_buf<B: Buf>(&mut self, buf: &mut B) -> Poll<usize, io::Error> {
            self.inner.write_buf(buf)
        }
    }
}
