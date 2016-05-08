//! A collection of traits abstracting over Listeners and Streams.
use std::any::{Any, TypeId};
use std::fmt;
use std::io::{self, ErrorKind, Read, Write};
use std::net::{SocketAddr, ToSocketAddrs, TcpStream, TcpListener, Shutdown};
use std::mem;

#[cfg(feature = "openssl")]
pub use self::openssl::{Openssl, OpensslClient};

use std::time::Duration;

use typeable::Typeable;
use traitobject;

/// The write-status indicating headers have not been written.
pub enum Fresh {}

/// The write-status indicating headers have been written.
pub enum Streaming {}

/// An abstraction to listen for connections on a certain port.
pub trait NetworkListener: Clone {
    /// The stream produced for each connection.
    type Stream: NetworkStream + Send + Clone;

    /// Returns an iterator of streams.
    fn accept(&mut self) -> ::Result<Self::Stream>;

    /// Get the address this Listener ended up listening on.
    fn local_addr(&mut self) -> io::Result<SocketAddr>;

    /// Returns an iterator over incoming connections.
    fn incoming(&mut self) -> NetworkConnections<Self> {
        NetworkConnections(self)
    }
}

/// An iterator wrapper over a `NetworkAcceptor`.
pub struct NetworkConnections<'a, N: NetworkListener + 'a>(&'a mut N);

impl<'a, N: NetworkListener + 'a> Iterator for NetworkConnections<'a, N> {
    type Item = ::Result<N::Stream>;
    fn next(&mut self) -> Option<::Result<N::Stream>> {
        Some(self.0.accept())
    }
}

/// An abstraction over streams that a `Server` can utilize.
pub trait NetworkStream: Read + Write + Any + Send + Typeable {
    /// Get the remote address of the underlying connection.
    fn peer_addr(&mut self) -> io::Result<SocketAddr>;

    /// Set the maximum time to wait for a read to complete.
    fn set_read_timeout(&self, dur: Option<Duration>) -> io::Result<()>;

    /// Set the maximum time to wait for a write to complete.
    fn set_write_timeout(&self, dur: Option<Duration>) -> io::Result<()>;

    /// This will be called when Stream should no longer be kept alive.
    #[inline]
    fn close(&mut self, _how: Shutdown) -> io::Result<()> {
        Ok(())
    }

    // Unsure about name and implementation...

    #[doc(hidden)]
    fn set_previous_response_expected_no_content(&mut self, _expected: bool) { }

    #[doc(hidden)]
    fn previous_response_expected_no_content(&self) -> bool {
        false
    }
}

/// A connector creates a NetworkStream.
pub trait NetworkConnector {
    /// Type of `Stream` to create
    type Stream: Into<Box<NetworkStream + Send>>;

    /// Connect to a remote address.
    fn connect(&self, host: &str, port: u16, scheme: &str) -> ::Result<Self::Stream>;
}

impl<T: NetworkStream + Send> From<T> for Box<NetworkStream + Send> {
    fn from(s: T) -> Box<NetworkStream + Send> {
        Box::new(s)
    }
}

impl fmt::Debug for Box<NetworkStream + Send> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.pad("Box<NetworkStream>")
    }
}

impl NetworkStream {
    unsafe fn downcast_ref_unchecked<T: 'static>(&self) -> &T {
        mem::transmute(traitobject::data(self))
    }

    unsafe fn downcast_mut_unchecked<T: 'static>(&mut self) -> &mut T {
        mem::transmute(traitobject::data_mut(self))
    }

    unsafe fn downcast_unchecked<T: 'static>(self: Box<NetworkStream>) -> Box<T>  {
        let raw: *mut NetworkStream = mem::transmute(self);
        mem::transmute(traitobject::data_mut(raw))
    }
}

impl NetworkStream {
    /// Is the underlying type in this trait object a `T`?
    #[inline]
    pub fn is<T: Any>(&self) -> bool {
        (*self).get_type() == TypeId::of::<T>()
    }

    /// If the underlying type is `T`, get a reference to the contained data.
    #[inline]
    pub fn downcast_ref<T: Any>(&self) -> Option<&T> {
        if self.is::<T>() {
            Some(unsafe { self.downcast_ref_unchecked() })
        } else {
            None
        }
    }

    /// If the underlying type is `T`, get a mutable reference to the contained
    /// data.
    #[inline]
    pub fn downcast_mut<T: Any>(&mut self) -> Option<&mut T> {
        if self.is::<T>() {
            Some(unsafe { self.downcast_mut_unchecked() })
        } else {
            None
        }
    }

    /// If the underlying type is `T`, extract it.
    #[inline]
    pub fn downcast<T: Any>(self: Box<NetworkStream>)
            -> Result<Box<T>, Box<NetworkStream>> {
        if self.is::<T>() {
            Ok(unsafe { self.downcast_unchecked() })
        } else {
            Err(self)
        }
    }
}

impl NetworkStream + Send {
    unsafe fn downcast_ref_unchecked<T: 'static>(&self) -> &T {
        mem::transmute(traitobject::data(self))
    }

    unsafe fn downcast_mut_unchecked<T: 'static>(&mut self) -> &mut T {
        mem::transmute(traitobject::data_mut(self))
    }

    unsafe fn downcast_unchecked<T: 'static>(self: Box<NetworkStream + Send>) -> Box<T>  {
        let raw: *mut NetworkStream = mem::transmute(self);
        mem::transmute(traitobject::data_mut(raw))
    }
}

impl NetworkStream + Send {
    /// Is the underlying type in this trait object a `T`?
    #[inline]
    pub fn is<T: Any>(&self) -> bool {
        (*self).get_type() == TypeId::of::<T>()
    }

    /// If the underlying type is `T`, get a reference to the contained data.
    #[inline]
    pub fn downcast_ref<T: Any>(&self) -> Option<&T> {
        if self.is::<T>() {
            Some(unsafe { self.downcast_ref_unchecked() })
        } else {
            None
        }
    }

    /// If the underlying type is `T`, get a mutable reference to the contained
    /// data.
    #[inline]
    pub fn downcast_mut<T: Any>(&mut self) -> Option<&mut T> {
        if self.is::<T>() {
            Some(unsafe { self.downcast_mut_unchecked() })
        } else {
            None
        }
    }

    /// If the underlying type is `T`, extract it.
    #[inline]
    pub fn downcast<T: Any>(self: Box<NetworkStream + Send>)
            -> Result<Box<T>, Box<NetworkStream + Send>> {
        if self.is::<T>() {
            Ok(unsafe { self.downcast_unchecked() })
        } else {
            Err(self)
        }
    }
}

/// A `NetworkListener` for `HttpStream`s.
pub struct HttpListener(TcpListener);

impl Clone for HttpListener {
    #[inline]
    fn clone(&self) -> HttpListener {
        HttpListener(self.0.try_clone().unwrap())
    }
}

impl From<TcpListener> for HttpListener {
    fn from(listener: TcpListener) -> HttpListener {
        HttpListener(listener)
    }
}

impl HttpListener {
    /// Start listening to an address over HTTP.
    pub fn new<To: ToSocketAddrs>(addr: To) -> ::Result<HttpListener> {
        Ok(HttpListener(try!(TcpListener::bind(addr))))
    }
}

impl NetworkListener for HttpListener {
    type Stream = HttpStream;

    #[inline]
    fn accept(&mut self) -> ::Result<HttpStream> {
        Ok(HttpStream(try!(self.0.accept()).0))
    }

    #[inline]
    fn local_addr(&mut self) -> io::Result<SocketAddr> {
        self.0.local_addr()
    }
}

#[cfg(windows)]
impl ::std::os::windows::io::AsRawSocket for HttpListener {
    fn as_raw_socket(&self) -> ::std::os::windows::io::RawSocket {
        self.0.as_raw_socket()
    }
}

#[cfg(windows)]
impl ::std::os::windows::io::FromRawSocket for HttpListener {
    unsafe fn from_raw_socket(sock: ::std::os::windows::io::RawSocket) -> HttpListener {
        HttpListener(TcpListener::from_raw_socket(sock))
    }
}

#[cfg(unix)]
impl ::std::os::unix::io::AsRawFd for HttpListener {
    fn as_raw_fd(&self) -> ::std::os::unix::io::RawFd {
        self.0.as_raw_fd()
    }
}

#[cfg(unix)]
impl ::std::os::unix::io::FromRawFd for HttpListener {
    unsafe fn from_raw_fd(fd: ::std::os::unix::io::RawFd) -> HttpListener {
        HttpListener(TcpListener::from_raw_fd(fd))
    }
}

/// A wrapper around a `TcpStream`.
pub struct HttpStream(pub TcpStream);

impl Clone for HttpStream {
    #[inline]
    fn clone(&self) -> HttpStream {
        HttpStream(self.0.try_clone().unwrap())
    }
}

impl fmt::Debug for HttpStream {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("HttpStream(_)")
    }
}

impl Read for HttpStream {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.read(buf)
    }
}

impl Write for HttpStream {
    #[inline]
    fn write(&mut self, msg: &[u8]) -> io::Result<usize> {
        self.0.write(msg)
    }
    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

#[cfg(windows)]
impl ::std::os::windows::io::AsRawSocket for HttpStream {
    fn as_raw_socket(&self) -> ::std::os::windows::io::RawSocket {
        self.0.as_raw_socket()
    }
}

#[cfg(windows)]
impl ::std::os::windows::io::FromRawSocket for HttpStream {
    unsafe fn from_raw_socket(sock: ::std::os::windows::io::RawSocket) -> HttpStream {
        HttpStream(TcpStream::from_raw_socket(sock))
    }
}

#[cfg(unix)]
impl ::std::os::unix::io::AsRawFd for HttpStream {
    fn as_raw_fd(&self) -> ::std::os::unix::io::RawFd {
        self.0.as_raw_fd()
    }
}

#[cfg(unix)]
impl ::std::os::unix::io::FromRawFd for HttpStream {
    unsafe fn from_raw_fd(fd: ::std::os::unix::io::RawFd) -> HttpStream {
        HttpStream(TcpStream::from_raw_fd(fd))
    }
}

impl NetworkStream for HttpStream {
    #[inline]
    fn peer_addr(&mut self) -> io::Result<SocketAddr> {
            self.0.peer_addr()
    }

    #[inline]
    fn set_read_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        self.0.set_read_timeout(dur)
    }

    #[inline]
    fn set_write_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        self.0.set_write_timeout(dur)
    }

    #[inline]
    fn close(&mut self, how: Shutdown) -> io::Result<()> {
        match self.0.shutdown(how) {
            Ok(_) => Ok(()),
            // see https://github.com/hyperium/hyper/issues/508
            Err(ref e) if e.kind() == ErrorKind::NotConnected => Ok(()),
            err => err
        }
    }
}

/// A connector that will produce HttpStreams.
#[derive(Debug, Clone, Default)]
pub struct HttpConnector;

impl NetworkConnector for HttpConnector {
    type Stream = HttpStream;

    fn connect(&self, host: &str, port: u16, scheme: &str) -> ::Result<HttpStream> {
        let addr = &(host, port);
        Ok(try!(match scheme {
            "http" => {
                debug!("http scheme");
                Ok(HttpStream(try!(TcpStream::connect(addr))))
            },
            _ => {
                Err(io::Error::new(io::ErrorKind::InvalidInput,
                                "Invalid scheme for Http"))
            }
        }))
    }
}

/// A closure as a connector used to generate `TcpStream`s per request
///
/// # Example
///
/// Basic example:
///
/// ```norun
/// Client::with_connector(|addr: &str, port: u16, scheme: &str| {
///     TcpStream::connect(&(addr, port))
/// });
/// ```
///
/// Example using `TcpBuilder` from the net2 crate if you want to configure your source socket:
///
/// ```norun
/// Client::with_connector(|addr: &str, port: u16, scheme: &str| {
///     let b = try!(TcpBuilder::new_v4());
///     try!(b.bind("127.0.0.1:0"));
///     b.connect(&(addr, port))
/// });
/// ```
impl<F> NetworkConnector for F where F: Fn(&str, u16, &str) -> io::Result<TcpStream> {
    type Stream = HttpStream;

    fn connect(&self, host: &str, port: u16, scheme: &str) -> ::Result<HttpStream> {
        Ok(HttpStream(try!((*self)(host, port, scheme))))
    }
}

/// Deprecated
///
/// Use `SslClient` and `SslServer` instead.
pub trait Ssl {
    /// The protected stream.
    type Stream: NetworkStream + Send + Clone;
    /// Wrap a client stream with SSL.
    fn wrap_client(&self, stream: HttpStream, host: &str) -> ::Result<Self::Stream>;
    /// Wrap a server stream with SSL.
    fn wrap_server(&self, stream: HttpStream) -> ::Result<Self::Stream>;
}

/// An abstraction to allow any SSL implementation to be used with client-side HttpsStreams.
pub trait SslClient<T: NetworkStream + Send + Clone = HttpStream> {
    /// The protected stream.
    type Stream: NetworkStream + Send + Clone;
    /// Wrap a client stream with SSL.
    fn wrap_client(&self, stream: T, host: &str) -> ::Result<Self::Stream>;
}

/// An abstraction to allow any SSL implementation to be used with server-side HttpsStreams.
pub trait SslServer<T: NetworkStream + Send + Clone = HttpStream> {
    /// The protected stream.
    type Stream: NetworkStream + Send + Clone;
    /// Wrap a server stream with SSL.
    fn wrap_server(&self, stream: T) -> ::Result<Self::Stream>;
}

impl<S: Ssl> SslClient<HttpStream> for S {
    type Stream = <S as Ssl>::Stream;

    fn wrap_client(&self, stream: HttpStream, host: &str) -> ::Result<Self::Stream> {
        Ssl::wrap_client(self, stream, host)
    }
}

impl<S: Ssl> SslServer<HttpStream> for S {
    type Stream = <S as Ssl>::Stream;

    fn wrap_server(&self, stream: HttpStream) -> ::Result<Self::Stream> {
        Ssl::wrap_server(self, stream)
    }
}

/// A stream over the HTTP protocol, possibly protected by SSL.
#[derive(Debug, Clone)]
pub enum HttpsStream<S: NetworkStream> {
    /// A plain text stream.
    Http(HttpStream),
    /// A stream protected by SSL.
    Https(S)
}

impl<S: NetworkStream> Read for HttpsStream<S> {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match *self {
            HttpsStream::Http(ref mut s) => s.read(buf),
            HttpsStream::Https(ref mut s) => s.read(buf)
        }
    }
}

impl<S: NetworkStream> Write for HttpsStream<S> {
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

impl<S: NetworkStream> NetworkStream for HttpsStream<S> {
    #[inline]
    fn peer_addr(&mut self) -> io::Result<SocketAddr> {
        match *self {
            HttpsStream::Http(ref mut s) => s.peer_addr(),
            HttpsStream::Https(ref mut s) => s.peer_addr()
        }
    }

    #[inline]
    fn set_read_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        match *self {
            HttpsStream::Http(ref inner) => inner.0.set_read_timeout(dur),
            HttpsStream::Https(ref inner) => inner.set_read_timeout(dur)
        }
    }

    #[inline]
    fn set_write_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        match *self {
            HttpsStream::Http(ref inner) => inner.0.set_write_timeout(dur),
            HttpsStream::Https(ref inner) => inner.set_write_timeout(dur)
        }
    }

    #[inline]
    fn close(&mut self, how: Shutdown) -> io::Result<()> {
        match *self {
            HttpsStream::Http(ref mut s) => s.close(how),
            HttpsStream::Https(ref mut s) => s.close(how)
        }
    }
}

/// A Http Listener over SSL.
#[derive(Clone)]
pub struct HttpsListener<S: SslServer> {
    listener: HttpListener,
    ssl: S,
}

impl<S: Ssl> HttpsListener<S> {
    /// Start listening to an address over HTTPS.
    pub fn new<To: ToSocketAddrs>(addr: To, ssl: S) -> ::Result<HttpsListener<S>> {
        HttpListener::new(addr).map(|l| HttpsListener {
            listener: l,
            ssl: ssl
        })
    }

    /// Construct an HttpsListener from a bound `TcpListener`.
    pub fn with_listener(listener: HttpListener, ssl: S) -> HttpsListener<S> {
        HttpsListener {
            listener: listener,
            ssl: ssl
        }
    }
}

impl<S: SslServer + Clone> NetworkListener for HttpsListener<S> {
    type Stream = S::Stream;

    #[inline]
    fn accept(&mut self) -> ::Result<S::Stream> {
        self.listener.accept().and_then(|s| self.ssl.wrap_server(s))
    }

    #[inline]
    fn local_addr(&mut self) -> io::Result<SocketAddr> {
        self.listener.local_addr()
    }
}

/// A connector that can protect HTTP streams using SSL.
#[derive(Debug, Default)]
pub struct HttpsConnector<S: SslClient, C: NetworkConnector = HttpConnector> {
    ssl: S,
    connector: C,
}

impl<S: SslClient> HttpsConnector<S, HttpConnector> {
    /// Create a new connector using the provided SSL implementation.
    pub fn new(s: S) -> HttpsConnector<S, HttpConnector> {
        HttpsConnector::with_connector(s, HttpConnector)
    }
}

impl<S: SslClient, C: NetworkConnector> HttpsConnector<S, C> {
    /// Create a new connector using the provided SSL implementation.
    pub fn with_connector(s: S, connector: C) -> HttpsConnector<S, C> {
        HttpsConnector { ssl: s, connector: connector }
    }
}

impl<S: SslClient, C: NetworkConnector<Stream=HttpStream>> NetworkConnector for HttpsConnector<S, C> {
    type Stream = HttpsStream<S::Stream>;

    fn connect(&self, host: &str, port: u16, scheme: &str) -> ::Result<Self::Stream> {
        let stream = try!(self.connector.connect(host, port, "http"));
        if scheme == "https" {
            debug!("https scheme");
            self.ssl.wrap_client(stream, host).map(HttpsStream::Https)
        } else {
            Ok(HttpsStream::Http(stream))
        }
    }
}


#[cfg(all(not(feature = "openssl"), not(feature = "security-framework")))]
#[doc(hidden)]
pub type DefaultConnector = HttpConnector;

#[cfg(feature = "openssl")]
#[doc(hidden)]
pub type DefaultConnector = HttpsConnector<self::openssl::Openssl>;

#[cfg(all(feature = "security-framework", not(feature = "openssl")))]
pub type DefaultConnector = HttpsConnector<self::security_framework::ClientWrapper>;

#[cfg(feature = "openssl")]
mod openssl {
    use std::io;
    use std::net::{SocketAddr, Shutdown};
    use std::path::Path;
    use std::sync::Arc;
    use std::time::Duration;

    use openssl::ssl::{Ssl, SslContext, SslStream, SslMethod, SSL_VERIFY_NONE, SSL_VERIFY_PEER, SSL_OP_NO_SSLV2, SSL_OP_NO_SSLV3};
    use openssl::ssl::error::StreamError as SslIoError;
    use openssl::ssl::error::SslError;
    use openssl::x509::X509FileType;
    use super::{NetworkStream, HttpStream};

    /// An implementation of `Ssl` for OpenSSL.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use hyper::Server;
    /// use hyper::net::Openssl;
    ///
    /// let ssl = Openssl::with_cert_and_key("/home/foo/cert", "/home/foo/key").unwrap();
    /// Server::https("0.0.0.0:443", ssl).unwrap();
    /// ```
    ///
    /// For complete control, create a `SslContext` with the options you desire
    /// and then create `Openssl { context: ctx }
    #[derive(Debug, Clone)]
    pub struct Openssl {
        /// The `SslContext` from openssl crate.
        pub context: Arc<SslContext>
    }

    /// A client-specific implementation of OpenSSL.
    #[derive(Debug, Clone)]
    pub struct OpensslClient(SslContext);

    impl Default for OpensslClient {
        fn default() -> OpensslClient {
            let mut ctx = SslContext::new(SslMethod::Sslv23).unwrap();
            ctx.set_default_verify_paths().unwrap();
            ctx.set_options(SSL_OP_NO_SSLV2 | SSL_OP_NO_SSLV3);
            OpensslClient(ctx)
        }
    }


    impl<T: NetworkStream + Send + Clone> super::SslClient<T> for OpensslClient {
        type Stream = SslStream<T>;

        fn wrap_client(&self, stream: T, host: &str) -> ::Result<Self::Stream> {
            let mut ssl = try!(Ssl::new(&self.0));
            try!(ssl.set_hostname(host));
            let host = host.to_owned();
            ssl.set_verify_callback(SSL_VERIFY_PEER, move |p, x| ::openssl_verify::verify_callback(&host, p, x));
            SslStream::connect(ssl, stream).map_err(From::from)
        }
    }

    impl Default for Openssl {
        fn default() -> Openssl {
            Openssl {
                context: Arc::new(SslContext::new(SslMethod::Sslv23).unwrap_or_else(|e| {
                    // if we cannot create a SslContext, that's because of a
                    // serious problem. just crash.
                    panic!("{}", e)
                }))
            }
        }
    }

    impl Openssl {
        /// Ease creating an `Openssl` with a certificate and key.
        pub fn with_cert_and_key<C, K>(cert: C, key: K) -> Result<Openssl, SslError>
        where C: AsRef<Path>, K: AsRef<Path> {
            let mut ctx = try!(SslContext::new(SslMethod::Sslv23));
            try!(ctx.set_cipher_list("DEFAULT"));
            try!(ctx.set_certificate_file(cert.as_ref(), X509FileType::PEM));
            try!(ctx.set_private_key_file(key.as_ref(), X509FileType::PEM));
            ctx.set_verify(SSL_VERIFY_NONE, None);
            Ok(Openssl { context: Arc::new(ctx) })
        }
    }

    impl super::Ssl for Openssl {
        type Stream = SslStream<HttpStream>;

        fn wrap_client(&self, stream: HttpStream, host: &str) -> ::Result<Self::Stream> {
            let ssl = try!(Ssl::new(&self.context));
            try!(ssl.set_hostname(host));
            SslStream::connect(ssl, stream).map_err(From::from)
        }

        fn wrap_server(&self, stream: HttpStream) -> ::Result<Self::Stream> {
            match SslStream::accept(&*self.context, stream) {
                Ok(ssl_stream) => Ok(ssl_stream),
                Err(SslIoError(e)) => {
                    Err(io::Error::new(io::ErrorKind::ConnectionAborted, e).into())
                },
                Err(e) => Err(e.into())
            }
        }
    }

    impl<S: NetworkStream> NetworkStream for SslStream<S> {
        #[inline]
        fn peer_addr(&mut self) -> io::Result<SocketAddr> {
            self.get_mut().peer_addr()
        }

        #[inline]
        fn set_read_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
            self.get_ref().set_read_timeout(dur)
        }

        #[inline]
        fn set_write_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
            self.get_ref().set_write_timeout(dur)
        }

        fn close(&mut self, how: Shutdown) -> io::Result<()> {
            self.get_mut().close(how)
        }
    }
}

#[cfg(feature = "security-framework")]
pub mod security_framework {
    use std::io;
    use std::fmt;
    use std::sync::{Arc, Mutex};
    use std::net::{Shutdown, SocketAddr};
    use std::time::Duration;
    use security_framework::secure_transport::{SslStream, ClientBuilder, ServerBuilder};

    use error::Error;
    use net::{SslClient, SslServer, HttpStream, NetworkStream};

    #[derive(Default)]
    pub struct ClientWrapper(ClientBuilder);

    impl ClientWrapper {
        pub fn new(builder: ClientBuilder) -> ClientWrapper {
            ClientWrapper(builder)
        }
    }

    impl SslClient for ClientWrapper {
        type Stream = Stream;

        fn wrap_client(&self, stream: HttpStream, host: &str) -> ::Result<Stream> {
            match self.0.handshake(host, stream) {
                Ok(s) => Ok(Stream(Arc::new(Mutex::new(s)))),
                Err(e) => Err(Error::Ssl(e.into())),
            }
        }
    }

    #[derive(Clone)]
    pub struct ServerWrapper(Arc<ServerBuilder>);

    impl ServerWrapper {
        pub fn new(builder: ServerBuilder) -> ServerWrapper {
            ServerWrapper(Arc::new(builder))
        }
    }

    impl SslServer for ServerWrapper {
        type Stream = Stream;

        fn wrap_server(&self, stream: HttpStream) -> ::Result<Stream> {
            match self.0.handshake(stream) {
                Ok(s) => Ok(Stream(Arc::new(Mutex::new(s)))),
                Err(e) => Err(Error::Ssl(e.into())),
            }
        }
    }

    #[derive(Clone)]
    pub struct Stream(Arc<Mutex<SslStream<HttpStream>>>);

    impl io::Read for Stream {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            self.0.lock().unwrap_or_else(|e| e.into_inner()).read(buf)
        }

        fn read_to_end(&mut self, buf: &mut Vec<u8>) -> io::Result<usize> {
            self.0.lock().unwrap_or_else(|e| e.into_inner()).read_to_end(buf)
        }

        fn read_to_string(&mut self, buf: &mut String) -> io::Result<usize> {
            self.0.lock().unwrap_or_else(|e| e.into_inner()).read_to_string(buf)
        }

        fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
            self.0.lock().unwrap_or_else(|e| e.into_inner()).read_exact(buf)
        }
    }

    impl io::Write for Stream {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.lock().unwrap_or_else(|e| e.into_inner()).write(buf)
        }

        fn flush(&mut self) -> io::Result<()> {
            self.0.lock().unwrap_or_else(|e| e.into_inner()).flush()
        }

        fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
            self.0.lock().unwrap_or_else(|e| e.into_inner()).write_all(buf)
        }

        fn write_fmt(&mut self, fmt: fmt::Arguments) -> io::Result<()> {
            self.0.lock().unwrap_or_else(|e| e.into_inner()).write_fmt(fmt)
        }
    }

    impl NetworkStream for Stream {
        fn peer_addr(&mut self) -> io::Result<SocketAddr> {
            self.0.lock().unwrap_or_else(|e| e.into_inner()).get_mut().peer_addr()
        }

        fn set_read_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
            self.0.lock().unwrap_or_else(|e| e.into_inner()).get_mut().set_read_timeout(dur)
        }

        fn set_write_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
            self.0.lock().unwrap_or_else(|e| e.into_inner()).get_mut().set_write_timeout(dur)
        }

        fn close(&mut self, how: Shutdown) -> io::Result<()> {
            self.0.lock().unwrap_or_else(|e| e.into_inner()).get_mut().close(how)
        }
    }
}

#[cfg(test)]
mod tests {
    use mock::MockStream;
    use super::{NetworkStream};

    #[test]
    fn test_downcast_box_stream() {
        // FIXME: Use Type ascription
        let stream: Box<NetworkStream + Send> = Box::new(MockStream::new());

        let mock = stream.downcast::<MockStream>().ok().unwrap();
        assert_eq!(mock, Box::new(MockStream::new()));
    }

    #[test]
    fn test_downcast_unchecked_box_stream() {
        // FIXME: Use Type ascription
        let stream: Box<NetworkStream + Send> = Box::new(MockStream::new());

        let mock = unsafe { stream.downcast_unchecked::<MockStream>() };
        assert_eq!(mock, Box::new(MockStream::new()));
    }
}

