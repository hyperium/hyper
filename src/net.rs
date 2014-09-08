//! A collection of traits abstracting over Listeners and Streams.
use std::io::{IoResult, Stream, Listener, Acceptor};
use std::io::net::ip::{SocketAddr, Port};
use std::io::net::tcp::{TcpStream, TcpListener, TcpAcceptor};

/// An abstraction to listen for connections on a certain port.
pub trait NetworkListener<S: NetworkStream, A: NetworkAcceptor<S>>: Listener<S, A> {
    /// Bind to a socket.
    ///
    /// Note: This does not start listening for connections. You must call
    /// `listen()` to do that.
    fn bind(host: &str, port: Port) -> IoResult<Self>;

    /// Get the address this Listener ended up listening on.
    fn socket_name(&mut self) -> IoResult<SocketAddr>;
}

/// An abstraction to receive `HttpStream`s.
pub trait NetworkAcceptor<S: NetworkStream>: Acceptor<S> + Clone + Send {
    /// Closes the Acceptor, so no more incoming connections will be handled.
    fn close(&mut self) -> IoResult<()>;
}

/// An abstraction over streams that a Server can utilize.
pub trait NetworkStream: Stream + Clone {
    /// Get the remote address of the underlying connection.
    fn peer_name(&mut self) -> IoResult<SocketAddr>;

    /// Connect to a remote address.
    fn connect(host: &str, port: Port) -> IoResult<Self>;
}

/// A `NetworkListener` for `HttpStream`s.
pub struct HttpListener {
    inner: TcpListener
}

impl Listener<HttpStream, HttpAcceptor> for HttpListener {
    #[inline]
    fn listen(self) -> IoResult<HttpAcceptor> {
        Ok(HttpAcceptor {
            inner: try!(self.inner.listen())
        })
    }
}

impl NetworkListener<HttpStream, HttpAcceptor> for HttpListener {
    #[inline]
    fn bind(host: &str, port: Port) -> IoResult<HttpListener> {
        Ok(HttpListener {
            inner: try!(TcpListener::bind(host, port))
        })
    }

    #[inline]
    fn socket_name(&mut self) -> IoResult<SocketAddr> {
        self.inner.socket_name()
    }
}

/// A `NetworkAcceptor` for `HttpStream`s.
#[deriving(Clone)]
pub struct HttpAcceptor {
    inner: TcpAcceptor
}

impl Acceptor<HttpStream> for HttpAcceptor {
    #[inline]
    fn accept(&mut self) -> IoResult<HttpStream> {
        Ok(HttpStream {
            inner: try!(self.inner.accept())
        })
    }
}

impl NetworkAcceptor<HttpStream> for HttpAcceptor {
    #[inline]
    fn close(&mut self) -> IoResult<()> {
        self.inner.close_accept()
    }
}

/// A wrapper around a TcpStream.
#[deriving(Clone)]
pub struct HttpStream {
    inner: TcpStream
}

impl Reader for HttpStream {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> IoResult<uint> {
        self.inner.read(buf)
    }
}

impl Writer for HttpStream {
    #[inline]
    fn write(&mut self, msg: &[u8]) -> IoResult<()> {
        self.inner.write(msg)
    }
    #[inline]
    fn flush(&mut self) -> IoResult<()> {
        self.inner.flush()
    }
}


impl NetworkStream for HttpStream {
    #[inline]
    fn peer_name(&mut self) -> IoResult<SocketAddr> {
        self.inner.peer_name()
    }

    #[inline]
    fn connect(host: &str, port: Port) -> IoResult<HttpStream> {
        Ok(HttpStream {
            inner: try!(TcpStream::connect(host, port))
        })
    }
}
