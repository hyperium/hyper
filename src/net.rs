//! A collection of traits abstracting over Listeners and Streams.
use std::any::Any;
use std::fmt;
use std::intrinsics::TypeId;
use std::io::{IoResult, IoError, ConnectionAborted, InvalidInput, OtherIoError,
              Stream, Listener, Acceptor};
use std::io::net::ip::{SocketAddr, ToSocketAddr, Port};
use std::io::net::tcp::{TcpStream, TcpListener, TcpAcceptor};
use std::mem;
use std::raw::{self, TraitObject};

use uany::UnsafeAnyExt;
use openssl::ssl::{Ssl, SslStream, SslContext, VerifyCallback};
use openssl::ssl::SslVerifyMode::SslVerifyPeer;
use openssl::ssl::SslMethod::Sslv23;
use openssl::ssl::error::{SslError, StreamError, OpenSslErrors, SslSessionClosed};

use self::HttpStream::{Http, Https};

/// The write-status indicating headers have not been written.
#[allow(missing_copy_implementations)]
pub struct Fresh;

/// The write-status indicating headers have been written.
#[allow(missing_copy_implementations)]
pub struct Streaming;

/// An abstraction to listen for connections on a certain port.
pub trait NetworkListener<S: NetworkStream, A: NetworkAcceptor<S>>: Listener<S, A> {
    /// Bind to a socket.
    ///
    /// Note: This does not start listening for connections. You must call
    /// `listen()` to do that.
    fn bind<To: ToSocketAddr>(addr: To) -> IoResult<Self>;

    /// Get the address this Listener ended up listening on.
    fn socket_name(&mut self) -> IoResult<SocketAddr>;
}

/// An abstraction to receive `NetworkStream`s.
pub trait NetworkAcceptor<S: NetworkStream>: Acceptor<S> + Clone + Send {
    /// Closes the Acceptor, so no more incoming connections will be handled.
    fn close(&mut self) -> IoResult<()>;
}

/// An abstraction over streams that a Server can utilize.
pub trait NetworkStream: Stream + Any + StreamClone + Send {
    /// Get the remote address of the underlying connection.
    fn peer_name(&mut self) -> IoResult<SocketAddr>;
}

#[doc(hidden)]
pub trait StreamClone {
    fn clone_box(&self) -> Box<NetworkStream + Send>;
}

impl<T: NetworkStream + Send + Clone> StreamClone for T {
    #[inline]
    fn clone_box(&self) -> Box<NetworkStream + Send> {
        box self.clone()
    }
}

/// A connector creates a NetworkStream.
pub trait NetworkConnector<S: NetworkStream> {
    /// Connect to a remote address.
    fn connect(&mut self, host: &str, port: Port, scheme: &str) -> IoResult<S>;
}

impl fmt::Show for Box<NetworkStream + Send> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.pad("Box<NetworkStream>")
    }
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

impl<'a> Reader for &'a mut NetworkStream {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> IoResult<uint> { (**self).read(buf) }
}

impl<'a> Writer for &'a mut NetworkStream {
    #[inline]
    fn write(&mut self, msg: &[u8]) -> IoResult<()> { (**self).write(msg) }

    #[inline]
    fn flush(&mut self) -> IoResult<()> { (**self).flush() }
}

impl UnsafeAnyExt for NetworkStream {
    unsafe fn downcast_ref_unchecked<T: 'static>(&self) -> &T {
        mem::transmute(mem::transmute::<&NetworkStream,
                                        raw::TraitObject>(self).data)
    }

    unsafe fn downcast_mut_unchecked<T: 'static>(&mut self) -> &mut T {
        mem::transmute(mem::transmute::<&mut NetworkStream,
                                        raw::TraitObject>(self).data)
    }

    unsafe fn downcast_unchecked<T: 'static>(self: Box<NetworkStream>) -> Box<T>  {
        mem::transmute(mem::transmute::<Box<NetworkStream>,
                                        raw::TraitObject>(self).data)
    }
}

impl NetworkStream {
    /// Is the underlying type in this trait object a T?
    #[inline]
    pub fn is<T: 'static>(&self) -> bool {
        self.get_type_id() == TypeId::of::<T>()
    }

    /// If the underlying type is T, get a reference to the contained data.
    #[inline]
    pub fn downcast_ref<T: 'static>(&self) -> Option<&T> {
        if self.is::<T>() {
            Some(unsafe { self.downcast_ref_unchecked() })
        } else {
            None
        }
    }

    /// If the underlying type is T, get a mutable reference to the contained
    /// data.
    #[inline]
    pub fn downcast_mut<T: 'static>(&mut self) -> Option<&mut T> {
        if self.is::<T>() {
            Some(unsafe { self.downcast_mut_unchecked() })
        } else {
            None
        }
    }

    /// If the underlying type is T, extract it.
    pub fn downcast<T: 'static>(self: Box<NetworkStream>)
            -> Result<Box<T>, Box<NetworkStream>> {
        if self.is::<T>() {
            Ok(unsafe { self.downcast_unchecked() })
        } else {
            Err(self)
        }
    }
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
    fn bind<To: ToSocketAddr>(addr: To) -> IoResult<HttpListener> {
        Ok(HttpListener {
            inner: try!(TcpListener::bind(addr))
        })
    }

    #[inline]
    fn socket_name(&mut self) -> IoResult<SocketAddr> {
        self.inner.socket_name()
    }
}

/// A `NetworkAcceptor` for `HttpStream`s.
#[derive(Clone)]
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
#[derive(Clone)]
pub enum HttpStream {
    /// A stream over the HTTP protocol.
    Http(TcpStream),
    /// A stream over the HTTP protocol, protected by SSL.
    Https(SslStream<TcpStream>),
}

impl Reader for HttpStream {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> IoResult<uint> {
        match *self {
            Http(ref mut inner) => inner.read(buf),
            Https(ref mut inner) => inner.read(buf)
        }
    }
}

impl Writer for HttpStream {
    #[inline]
    fn write(&mut self, msg: &[u8]) -> IoResult<()> {
        match *self {
            Http(ref mut inner) => inner.write(msg),
            Https(ref mut inner) => inner.write(msg)
        }
    }
    #[inline]
    fn flush(&mut self) -> IoResult<()> {
        match *self {
            Http(ref mut inner) => inner.flush(),
            Https(ref mut inner) => inner.flush(),
        }
    }
}

impl NetworkStream for HttpStream {
    fn peer_name(&mut self) -> IoResult<SocketAddr> {
        match *self {
            Http(ref mut inner) => inner.peer_name(),
            Https(ref mut inner) => inner.get_mut().peer_name()
        }
    }
}

/// A connector that will produce HttpStreams.
#[allow(missing_copy_implementations)]
pub struct HttpConnector(pub Option<VerifyCallback>);

impl NetworkConnector<HttpStream> for HttpConnector {
    fn connect(&mut self, host: &str, port: Port, scheme: &str) -> IoResult<HttpStream> {
        let addr = (host, port);
        match scheme {
            "http" => {
                debug!("http scheme");
                Ok(Http(try!(TcpStream::connect(addr))))
            },
            "https" => {
                debug!("https scheme");
                let stream = try!(TcpStream::connect(addr));
                let mut context = try!(SslContext::new(Sslv23).map_err(lift_ssl_error));
                self.0.as_ref().map(|cb| context.set_verify(SslVerifyPeer, Some(*cb)));
                let ssl = try!(Ssl::new(&context).map_err(lift_ssl_error));
                try!(ssl.set_hostname(host).map_err(lift_ssl_error));
                let stream = try!(SslStream::new(&context, stream).map_err(lift_ssl_error));
                Ok(Https(stream))
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
}

fn lift_ssl_error(ssl: SslError) -> IoError {
    debug!("lift_ssl_error: {}", ssl);
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

#[cfg(test)]
mod tests {
    use std::boxed::BoxAny;
    use uany::UnsafeAnyExt;

    use mock::MockStream;
    use super::NetworkStream;

    #[test]
    fn test_downcast_box_stream() {
        let stream = box MockStream::new() as Box<NetworkStream + Send>;

        let mock = stream.downcast::<MockStream>().ok().unwrap();
        assert_eq!(mock, box MockStream::new());

    }

    #[test]
    fn test_downcast_unchecked_box_stream() {
        let stream = box MockStream::new() as Box<NetworkStream + Send>;

        let mock = unsafe { stream.downcast_unchecked::<MockStream>() };
        assert_eq!(mock, box MockStream::new());

    }

}
