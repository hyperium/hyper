//! A collection of traits abstracting over Listeners and Streams.
use std::any::{Any, TypeId};
use std::fmt;
use std::io::{self, Read, Write};
use std::net::{SocketAddr, ToSocketAddrs, TcpStream, TcpListener};
use std::mem;
use std::path::Path;
use std::raw::{self, TraitObject};
use std::sync::Arc;

use openssl::ssl::{Ssl, SslStream, SslContext};
use openssl::ssl::SslVerifyMode::SslVerifyNone;
use openssl::ssl::SslMethod::Sslv23;
use openssl::ssl::error::{SslError, StreamError, OpenSslErrors, SslSessionClosed};
use openssl::x509::X509FileType;

macro_rules! try_some {
    ($expr:expr) => (match $expr {
        Some(val) => { return Err(val); },
        _ => {}
    })
}

/// The write-status indicating headers have not been written.
pub enum Fresh {}

/// The write-status indicating headers have been written.
pub enum Streaming {}

/// An abstraction to listen for connections on a certain port.
pub trait NetworkListener: Clone {
    /// The stream produced for each connection.
    type Stream: NetworkStream + Send + Clone;
    /// Listens on a socket.
    //fn listen<To: ToSocketAddrs>(&mut self, addr: To) -> io::Result<Self::Acceptor>;

    /// Returns an iterator of streams.
    fn accept(&mut self) -> io::Result<Self::Stream>;

    /// Get the address this Listener ended up listening on.
    fn socket_addr(&mut self) -> io::Result<SocketAddr>;

    /// Closes the Acceptor, so no more incoming connections will be handled.
//    fn close(&mut self) -> io::Result<()>;

    /// Returns an iterator over incoming connections.
    fn incoming(&mut self) -> NetworkConnections<Self> {
        NetworkConnections(self)
    }
}

/// An iterator wrapper over a NetworkAcceptor.
pub struct NetworkConnections<'a, N: NetworkListener + 'a>(&'a mut N);

impl<'a, N: NetworkListener + 'a> Iterator for NetworkConnections<'a, N> {
    type Item = io::Result<N::Stream>;
    fn next(&mut self) -> Option<io::Result<N::Stream>> {
        Some(self.0.accept())
    }
}


/// An abstraction over streams that a Server can utilize.
pub trait NetworkStream: Read + Write + Any + StreamClone + Send {
    /// Get the remote address of the underlying connection.
    fn peer_addr(&mut self) -> io::Result<SocketAddr>;
}


#[doc(hidden)]
pub trait StreamClone {
    fn clone_box(&self) -> Box<NetworkStream + Send>;
}

impl<T: NetworkStream + Send + Clone> StreamClone for T {
    #[inline]
    fn clone_box(&self) -> Box<NetworkStream + Send> {
        Box::new(self.clone())
    }
}

/// A connector creates a NetworkStream.
pub trait NetworkConnector {
    /// Type of Stream to create
    type Stream: NetworkStream + Send;
    /// Connect to a remote address.
    fn connect(&mut self, host: &str, port: u16, scheme: &str) -> io::Result<Self::Stream>;
}

impl fmt::Debug for Box<NetworkStream + Send> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.pad("Box<NetworkStream>")
    }
}

impl Clone for Box<NetworkStream + Send> {
    #[inline]
    fn clone(&self) -> Box<NetworkStream + Send> { self.clone_box() }
}

impl NetworkStream {
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
pub enum HttpListener {
    /// Http variant.
    Http(TcpListener),
    /// Https variant. The two paths point to the certificate and key PEM files, in that order.
    Https(TcpListener, Arc<SslContext>)
}

impl Clone for HttpListener {
    fn clone(&self) -> HttpListener {
        match *self {
            HttpListener::Http(ref tcp) => HttpListener::Http(tcp.try_clone().unwrap()),
            HttpListener::Https(ref tcp, ref ssl) => HttpListener::Https(tcp.try_clone().unwrap(), ssl.clone()),
        }
    }
}

impl HttpListener {

    /// Start listening to an address over HTTP.
    pub fn http<To: ToSocketAddrs>(addr: &To) -> io::Result<HttpListener> {
        Ok(HttpListener::Http(try!(TcpListener::bind(addr))))
    }

    /// Start listening to an address over HTTPS.
    pub fn https<To: ToSocketAddrs>(addr: &To, cert: &Path, key: &Path) -> io::Result<HttpListener> {
        let mut ssl_context = try!(SslContext::new(Sslv23).map_err(lift_ssl_error));
        try_some!(ssl_context.set_cipher_list("DEFAULT").map(lift_ssl_error));
        try_some!(ssl_context.set_certificate_file(
                cert, X509FileType::PEM).map(lift_ssl_error));
        try_some!(ssl_context.set_private_key_file(
                key, X509FileType::PEM).map(lift_ssl_error));
        ssl_context.set_verify(SslVerifyNone, None);
        Ok(HttpListener::Https(try!(TcpListener::bind(addr)), Arc::new(ssl_context)))
    }
}

impl NetworkListener for HttpListener {
    type Stream = HttpStream;

    #[inline]
    fn accept(&mut self) -> io::Result<HttpStream> {
        Ok(match *self {
            HttpListener::Http(ref mut tcp) => HttpStream::Http(CloneTcpStream(try!(tcp.accept()).0)),
            HttpListener::Https(ref mut tcp, ref ssl_context) => {
                let stream = CloneTcpStream(try!(tcp.accept()).0);
                match SslStream::new_server(&**ssl_context, stream) {
                    Ok(ssl_stream) => HttpStream::Https(ssl_stream),
                    Err(StreamError(ref e)) => {
                        return Err(io::Error::new(io::ErrorKind::ConnectionAborted,
                                                "SSL Handshake Interrupted",
                                                Some(e.to_string())));
                    },
                    Err(e) => return Err(lift_ssl_error(e))
                }
            }
        })
    }

    #[inline]
    fn socket_addr(&mut self) -> io::Result<SocketAddr> {
        match *self {
            HttpListener::Http(ref mut tcp) => tcp.socket_addr(),
            HttpListener::Https(ref mut tcp, _) => tcp.socket_addr(),
        }
    }
}

#[doc(hidden)]
pub struct CloneTcpStream(TcpStream);

impl Clone for CloneTcpStream{
    #[inline]
    fn clone(&self) -> CloneTcpStream {
        CloneTcpStream(self.0.try_clone().unwrap())
    }
}

impl Read for CloneTcpStream {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.read(buf)
    }
}

impl Write for CloneTcpStream {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }
    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

/// A wrapper around a TcpStream.
#[derive(Clone)]
pub enum HttpStream {
    /// A stream over the HTTP protocol.
    Http(CloneTcpStream),
    /// A stream over the HTTP protocol, protected by SSL.
    Https(SslStream<CloneTcpStream>),
}

impl Read for HttpStream {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match *self {
            HttpStream::Http(ref mut inner) => inner.read(buf),
            HttpStream::Https(ref mut inner) => inner.read(buf)
        }
    }
}

impl Write for HttpStream {
    #[inline]
    fn write(&mut self, msg: &[u8]) -> io::Result<usize> {
        match *self {
            HttpStream::Http(ref mut inner) => inner.write(msg),
            HttpStream::Https(ref mut inner) => inner.write(msg)
        }
    }
    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        match *self {
            HttpStream::Http(ref mut inner) => inner.flush(),
            HttpStream::Https(ref mut inner) => inner.flush(),
        }
    }
}

impl NetworkStream for HttpStream {
    fn peer_addr(&mut self) -> io::Result<SocketAddr> {
        match *self {
            HttpStream::Http(ref mut inner) => inner.0.peer_addr(),
            HttpStream::Https(ref mut inner) => inner.get_mut().0.peer_addr()
        }
    }
}

/// A connector that will produce HttpStreams.
#[allow(missing_copy_implementations)]
pub struct HttpConnector<'v>(pub Option<ContextVerifier<'v>>);

/// A method that can set verification methods on an SSL context
pub type ContextVerifier<'v> = Box<FnMut(&mut SslContext) -> ()+'v>;

impl<'v> NetworkConnector for HttpConnector<'v> {
    type Stream = HttpStream;

    fn connect(&mut self, host: &str, port: u16, scheme: &str) -> io::Result<HttpStream> {
        let addr = &(host, port);
        match scheme {
            "http" => {
                debug!("http scheme");
                Ok(HttpStream::Http(CloneTcpStream(try!(TcpStream::connect(addr)))))
            },
            "https" => {
                debug!("https scheme");
                let stream = CloneTcpStream(try!(TcpStream::connect(addr)));
                let mut context = try!(SslContext::new(Sslv23).map_err(lift_ssl_error));
                if let Some(ref mut verifier) = self.0 {
                    verifier(&mut context);
                }
                let ssl = try!(Ssl::new(&context).map_err(lift_ssl_error));
                try!(ssl.set_hostname(host).map_err(lift_ssl_error));
                let stream = try!(SslStream::new(&context, stream).map_err(lift_ssl_error));
                Ok(HttpStream::Https(stream))
            },
            _ => {
                Err(io::Error::new(io::ErrorKind::InvalidInput,
                                "Invalid scheme for Http",
                                None))
            }
        }
    }
}

fn lift_ssl_error(ssl: SslError) -> io::Error {
    debug!("lift_ssl_error: {:?}", ssl);
    match ssl {
        StreamError(err) => err,
        SslSessionClosed => io::Error::new(io::ErrorKind::ConnectionAborted,
                                         "SSL Connection Closed",
                                         None),
        // Unfortunately throw this away. No way to support this
        // detail without a better Error abstraction.
        OpenSslErrors(errs) => io::Error::new(io::ErrorKind::Other,
                                         "Error in OpenSSL",
                                         Some(format!("{:?}", errs)))
    }
}

#[cfg(test)]
mod tests {
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
