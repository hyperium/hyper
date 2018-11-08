use std::fmt;
use std::io;
use std::os::unix::net::{SocketAddr
    as UnixSocketAddr, UnixListener as StdUnixListener};
use std::time::{Duration, Instant};

use futures::{Async, Future, Poll, Stream};
use tokio_reactor::Handle;
use tokio_uds::UnixListener;
use tokio_timer::Delay;
use std::path::Path;

use self::uds_stream::UdsStream;

/// A stream of connections from binding to an address.
#[must_use = "streams do nothing unless polled"]
pub struct UnixIncoming {
    addr: UnixSocketAddr,
    listener: UnixListener,
    sleep_on_errors: bool,
    timeout: Option<Delay>,
}

impl UnixIncoming {
    pub(super) fn new<P: AsRef<Path>>(path: P, handle: Option<&Handle>) -> ::Result<Self> {
        let std_listener = StdUnixListener::bind(path)
                .map_err(::Error::new_listen)?;

        if let Some(handle) = handle {
            UnixIncoming::from_std(std_listener, handle)
        } else {
            let handle = Handle::current();
            UnixIncoming::from_std(std_listener, &handle)
        }
    }

    pub(super) fn from_std(std_listener: StdUnixListener, handle: &Handle) -> ::Result<Self> {
        let listener = UnixListener::from_std(std_listener, &handle)
            .map_err(::Error::new_listen)?;
        let addr = listener.local_addr().map_err(::Error::new_listen)?;
        Ok(UnixIncoming {
            listener,
            addr: addr,
            sleep_on_errors: true,
            timeout: None,
        })
    }

    /// Get the local address bound to this listener.
    pub fn local_addr(&self) -> &UnixSocketAddr {
        &self.addr
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

impl Stream for UnixIncoming {
    // currently unnameable...
    type Item = UdsStream;
    type Error = ::std::io::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        // Check if a previous timeout is active that was set by IO errors.
        if let Some(ref mut to) = self.timeout {
            match to.poll() {
                Ok(Async::Ready(())) => {}
                Ok(Async::NotReady) => return Ok(Async::NotReady),
                Err(err) => {
                    error!("sleep timer error: {}", err);
                }
            }
        }
        self.timeout = None;
        loop {
            match self.listener.poll_accept() {
                Ok(Async::Ready((socket, addr))) => {
                    return Ok(Async::Ready(Some(UdsStream::new(socket, addr))));
                },
                Ok(Async::NotReady) => return Ok(Async::NotReady),
                Err(e) => {
                    // Connection errors can be ignored directly, continue by
                    // accepting the next request.
                    if is_connection_error(&e) {
                        debug!("accepted connection already errored: {}", e);
                        continue;
                    }

                    if self.sleep_on_errors {
                        // Sleep 1s.
                        let delay = Instant::now() + Duration::from_secs(1);
                        let mut timeout = Delay::new(delay);

                        match timeout.poll() {
                            Ok(Async::Ready(())) => {
                                // Wow, it's been a second already? Ok then...
                                error!("accept error: {}", e);
                                continue
                            },
                            Ok(Async::NotReady) => {
                                error!("accept error: {}", e);
                                self.timeout = Some(timeout);
                                return Ok(Async::NotReady);
                            },
                            Err(timer_err) => {
                                error!("couldn't sleep on error, timer error: {}", timer_err);
                                return Err(e);
                            }
                        }
                    } else {
                        return Err(e);
                    }
                },
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
    match e.kind() {
        io::ErrorKind::ConnectionRefused |
        io::ErrorKind::ConnectionAborted |
        io::ErrorKind::ConnectionReset => true,
        _ => false,
    }
}

impl fmt::Debug for UnixIncoming {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("UnixIncoming")
            .field("addr", &self.addr)
            .field("sleep_on_errors", &self.sleep_on_errors)
            .finish()
    }
}

mod uds_stream {
    use std::io::{self, Read, Write};
    use std::os::unix::net::SocketAddr;
    use bytes::{Buf, BufMut};
    use futures::Poll;
    use tokio_uds::UnixStream;
    use tokio_io::{AsyncRead, AsyncWrite};


    #[derive(Debug)]
    pub struct UdsStream {
        inner: UnixStream,
        pub(super) remote_addr: SocketAddr,
    }

    impl UdsStream {
        pub(super) fn new(unix: UnixStream, addr: SocketAddr) -> UdsStream {
            UdsStream {
                inner: unix,
                remote_addr: addr,
            }
        }

        /// Returns the remote (peer) address of this connection.
        #[inline]
        pub fn remote_addr(&self) -> &SocketAddr {
            &self.remote_addr
        }
    }

    impl Read for UdsStream {
        #[inline]
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            self.inner.read(buf)
        }
    }

    impl Write for UdsStream {
        #[inline]
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.inner.write(buf)
        }

        #[inline]
        fn flush(&mut self) -> io::Result<()> {
            // TcpStream::flush is a noop, so skip calling it...
            Ok(())
        }
    }

    impl AsyncRead for UdsStream {
        #[inline]
        unsafe fn prepare_uninitialized_buffer(&self, buf: &mut [u8]) -> bool {
            self.inner.prepare_uninitialized_buffer(buf)
        }

        #[inline]
        fn read_buf<B: BufMut>(&mut self, buf: &mut B) -> Poll<usize, io::Error> {
            self.inner.read_buf(buf)
        }
    }

    impl AsyncWrite for UdsStream {
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
