//! A collection of traits abstracting over Listeners and Streams.
use std::io::{IoResult, IoError, ConnectionAborted, InvalidInput, OtherIoError,
              Stream, Listener, Acceptor};
use std::io::net::ip::{SocketAddr, Port};
use std::io::net::tcp::{TcpStream, TcpListener, TcpAcceptor};
use std::sync::{Arc, Mutex};

use openssl::ssl::{SslStream, SslContext, Sslv23};
use openssl::ssl::error::{SslError, StreamError, OpenSslErrors, SslSessionClosed};

/// The write-status indicating headers have not been written.
pub struct Fresh;

/// The write-status indicating headers have been written.
pub struct Streaming;

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

/// An abstraction to receive `NetworkStream`s.
pub trait NetworkAcceptor<S: NetworkStream>: Acceptor<S> + Clone + Send {
    /// Closes the Acceptor, so no more incoming connections will be handled.
    fn close(&mut self) -> IoResult<()>;
}

/// An abstraction over streams that a Server can utilize.
pub trait NetworkStream: Stream + Clone + Send {
    /// Get the remote address of the underlying connection.
    fn peer_name(&mut self) -> IoResult<SocketAddr>;

    /// Connect to a remote address.
    fn connect(host: &str, Port, scheme: &str) -> IoResult<Self>;

    /// Turn this into an appropriately typed trait object.
    #[inline]
    fn abstract(self) -> Box<NetworkStream + Send> {
        box self as Box<NetworkStream + Send>
    }

    #[doc(hidden)]
    #[inline]
    // Hack to work around lack of Clone impl for Box<Clone>
    fn clone_box(&self) -> Box<NetworkStream + Send> { self.clone().abstract() }
}

impl Clone for Box<NetworkStream + Send> {
    #[inline]
    fn clone(&self) -> Box<NetworkStream + Send> { self.clone_box() }
}

impl Reader for Box<NetworkStream + Send> {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> IoResult<uint> { (**self).read(buf) }
}

impl Writer for Box<NetworkStream + Send> {
    #[inline]
    fn write(&mut self, msg: &[u8]) -> IoResult<()> { (**self).write(msg) }

    #[inline]
    fn flush(&mut self) -> IoResult<()> { (**self).flush() }
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
        Ok(Http(try!(self.inner.accept())))
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
pub enum HttpStream {
    /// A stream over the HTTP protocol.
    Http(TcpStream),
    /// A stream over the HTTP protocol, protected by SSL.
    // You may be asking wtf an Arc and Mutex? That's because SslStream
    // doesn't implement Clone, and we need Clone to use the stream for
    // both the Request and Response.
    // FIXME: https://github.com/sfackler/rust-openssl/issues/6
    Https(Arc<Mutex<SslStream<TcpStream>>>, SocketAddr),
}

impl Reader for HttpStream {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> IoResult<uint> {
        match *self {
            Http(ref mut inner) => inner.read(buf),
            Https(ref mut inner, _) => inner.lock().read(buf)
        }
    }
}

impl Writer for HttpStream {
    #[inline]
    fn write(&mut self, msg: &[u8]) -> IoResult<()> {
        match *self {
            Http(ref mut inner) => inner.write(msg),
            Https(ref mut inner, _) => inner.lock().write(msg)
        }
    }
    #[inline]
    fn flush(&mut self) -> IoResult<()> {
        match *self {
            Http(ref mut inner) => inner.flush(),
            Https(ref mut inner, _) => inner.lock().flush(),
        }
    }
}


impl NetworkStream for HttpStream {
    fn connect(host: &str, port: Port, scheme: &str) -> IoResult<HttpStream> {
        match scheme {
            "http" => {
                debug!("http scheme");
                Ok(Http(try!(TcpStream::connect(host, port))))
            },
            "https" => {
                debug!("https scheme");
                let mut stream = try!(TcpStream::connect(host, port));
                // we can't access the tcp stream once it's wrapped in an
                // SslStream, so grab the ip address now, just in case.
                let addr = try!(stream.peer_name());
                let context = try!(SslContext::new(Sslv23).map_err(lift_ssl_error));
                let stream = try!(SslStream::new(&context, stream).map_err(lift_ssl_error));
                Ok(Https(Arc::new(Mutex::new(stream)), addr))
            },
            _ => {
                Err(IoError {
                    kind: InvalidInput,
                    desc: "Invalid scheme for Http",
                    detail: None
                })
            }
        }
    }

    fn peer_name(&mut self) -> IoResult<SocketAddr> {
        match *self {
            Http(ref mut inner) => inner.peer_name(),
            Https(_, addr) => Ok(addr)
        }
    }
}

fn lift_ssl_error(ssl: SslError) -> IoError {
    match ssl {
        StreamError(err) => err,
        SslSessionClosed => IoError {
            kind: ConnectionAborted,
            desc: "SSL Connection Closed",
            detail: None
        },
        // Unfortunately throw this away. No way to support this
        // detail without a better Error abstraction.
        OpenSslErrors(errs) => IoError {
            kind: OtherIoError,
            desc: "Error in OpenSSL",
            detail: Some(format!("{}", errs))
        }
    }
}
