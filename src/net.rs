//! A collection of traits abstracting over Listeners and Streams.
use std::io::{self, Read, Write};
use std::net::{SocketAddr};
use std::option;

use std::net::{TcpStream, TcpListener};


/// An alias to `mio::tcp::TcpStream`.
//#[derive(Debug)]
pub struct HttpStream(pub ::tokio::net::TcpStream);

impl Read for HttpStream {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.read(buf)
    }
}

impl Write for HttpStream {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

/*
#[cfg(not(windows))]
impl ::vecio::Writev for HttpStream {
    #[inline]
    fn writev(&mut self, bufs: &[&[u8]]) -> io::Result<usize> {
        use ::vecio::Rawv;
        self.0.writev(bufs)
    }
}
*/

/// An alias to `mio::tcp::TcpListener`.
#[derive(Debug)]
pub struct HttpListener(pub TcpListener);

impl HttpListener {
    /// Bind to a socket address.
    pub fn bind(addr: &SocketAddr) -> io::Result<HttpListener> {
        TcpListener::bind(addr)
            .map(HttpListener)
    }

    /// Try to duplicate the underlying listening socket.
    pub fn try_clone(&self) -> io::Result<HttpListener> {
        self.0.try_clone().map(HttpListener)
    }
}

/*
/// An abstraction to allow any SSL implementation to be used with client-side `HttpsStream`s.
pub trait SslClient {
    /// The protected stream.
    type Stream: Transport;
    /// Wrap a client stream with SSL.
    fn wrap_client(&self, stream: HttpStream, host: &str) -> ::Result<Self::Stream>;
}

/// An abstraction to allow any SSL implementation to be used with server-side `HttpsStream`s.
pub trait SslServer {
    /// The protected stream.
    type Stream: Transport;
    /// Wrap a server stream with SSL.
    fn wrap_server(&self, stream: HttpStream) -> ::Result<Self::Stream>;
}


/// A stream over the HTTP protocol, possibly protected by TLS.
#[derive(Debug)]
pub enum HttpsStream<S: Transport> {
    /// A plain text stream.
    Http(HttpStream),
    /// A stream protected by TLS.
    Https(S)
}

impl<S: Transport> Read for HttpsStream<S> {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match *self {
            HttpsStream::Http(ref mut s) => s.read(buf),
            HttpsStream::Https(ref mut s) => s.read(buf)
        }
    }
}

impl<S: Transport> Write for HttpsStream<S> {
    #[inline]
    fn write(&mut self, msg: &[u8]) -> io::Result<usize> {
        match *self {
            HttpsStream::Http(ref mut s) => s.write(msg),
            HttpsStream::Https(ref mut s) => s.write(msg)
        }
    }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        match *self {
            HttpsStream::Http(ref mut s) => s.flush(),
            HttpsStream::Https(ref mut s) => s.flush()
        }
    }
}

/*
#[cfg(not(windows))]
impl<S: Transport> ::vecio::Writev for HttpsStream<S> {
    #[inline]
    fn writev(&mut self, bufs: &[&[u8]]) -> io::Result<usize> {
        match *self {
            HttpsStream::Http(ref mut s) => s.writev(bufs),
            HttpsStream::Https(ref mut s) => s.writev(bufs)
        }
    }
}
*/


/*
#[cfg(unix)]
impl ::std::os::unix::io::AsRawFd for HttpStream {
    #[inline]
    fn as_raw_fd(&self) -> ::std::os::unix::io::RawFd {
        self.0.as_raw_fd()
    }
}

#[cfg(unix)]
impl<S: Transport + ::std::os::unix::io::AsRawFd> ::std::os::unix::io::AsRawFd for HttpsStream<S> {
    #[inline]
    fn as_raw_fd(&self) -> ::std::os::unix::io::RawFd {
        match *self {
            HttpsStream::Http(ref s) => s.as_raw_fd(),
            HttpsStream::Https(ref s) => s.as_raw_fd(),
        }
    }
}
*/

/// An `HttpListener` over SSL.
#[derive(Debug)]
pub struct HttpsListener<S: SslServer> {
    listener: TcpListener,
    ssl: S,
}

impl<S: SslServer> HttpsListener<S> {
    /// Start listening to an address over HTTPS.
    #[inline]
    pub fn new(addr: &SocketAddr, ssl: S) -> io::Result<HttpsListener<S>> {
        TcpListener::bind(addr).map(|l| HttpsListener {
            listener: l,
            ssl: ssl
        })
    }

    /// Construct an `HttpsListener` from a bound `TcpListener`.
    pub fn with_listener(listener: TcpListener, ssl: S) -> HttpsListener<S> {
        HttpsListener {
            listener: listener,
            ssl: ssl
        }
    }
}

/*
impl<S: SslServer> Accept for HttpsListener<S> {
    type Output = S::Stream;

    #[inline]
    fn accept(&self) -> io::Result<Option<S::Stream>> {
        self.listener.accept().and_then(|s| match s {
            Some((s, _)) => self.ssl.wrap_server(HttpStream(s)).map(Some).map_err(|e| {
                match e {
                    ::Error::Io(e) => e,
                    _ => io::Error::new(io::ErrorKind::Other, e),

                }
            }),
            None => Ok(None),
        })
    }

    #[inline]
    fn local_addr(&self) -> io::Result<SocketAddr> {
        self.listener.local_addr()
    }
}
*/

*/
