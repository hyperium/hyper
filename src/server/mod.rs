//! HTTP Server
//!
//! # Example
//!
//! ```no_run
//! use hyper::server::{Server, Request, Response};
//! use hyper::status::StatusCode;
//! use hyper::uri::RequestUri;
//!
//! let server = Server::http(|req: Request, mut res: Response| {
//!     *res.status_mut() = match (req.method, req.uri) {
//!         (hyper::Get, RequestUri::AbsolutePath(ref path)) if path == "/"  => {
//!             StatusCode::Ok
//!         },
//!         (hyper::Get, _) => StatusCode::NotFound,
//!         _ => StatusCode::MethodNotAllowed
//!     };
//! }).listen("0.0.0.0:8080").unwrap();
use std::fmt;
use std::io::{ErrorKind, BufWriter, Write};
use std::marker::PhantomData;
use std::net::{SocketAddr, ToSocketAddrs};
use std::path::Path;
use std::thread::{self, JoinHandle};

use num_cpus;
use openssl::ssl::SslContext;

pub use self::request::Request;
pub use self::response::Response;

pub use net::{Fresh, Streaming};

use Error;
use buffer::BufReader;
use header::{Headers, Expect, Connection};
use http;
use method::Method;
use net::{NetworkListener, NetworkStream, HttpListener};
use status::StatusCode;
use uri::RequestUri;
use version::HttpVersion::Http11;

use self::listener::ListenerPool;

pub mod request;
pub mod response;

mod listener;

#[derive(Debug)]
enum SslConfig<'a> {
    CertAndKey(&'a Path, &'a Path),
    Context(SslContext),
}

/// A server can listen on a TCP socket.
///
/// Once listening, it will create a `Request`/`Response` pair for each
/// incoming connection, and hand them to the provided handler.
#[derive(Debug)]
pub struct Server<'a, H: Handler, L = HttpListener> {
    handler: H,
    ssl: Option<SslConfig<'a>>,
    _marker: PhantomData<L>
}

macro_rules! try_option(
    ($e:expr) => {{
        match $e {
            Some(v) => v,
            None => return None
        }
    }}
);

impl<'a, H: Handler, L: NetworkListener> Server<'a, H, L> {
    /// Creates a new server with the provided handler.
    pub fn new(handler: H) -> Server<'a, H, L> {
        Server {
            handler: handler,
            ssl: None,
            _marker: PhantomData
        }
    }
}

impl<'a, H: Handler + 'static> Server<'a, H, HttpListener> {
    /// Creates a new server that will handle `HttpStream`s.
    pub fn http(handler: H) -> Server<'a, H, HttpListener> {
        Server::new(handler)
    }
    /// Creates a new server that will handler `HttpStreams`s using a TLS connection.
    pub fn https(handler: H, cert: &'a Path, key: &'a Path) -> Server<'a, H, HttpListener> {
        Server {
            handler: handler,
            ssl: Some(SslConfig::CertAndKey(cert, key)),
            _marker: PhantomData
        }
    }
    /// Creates a new server that will handler `HttpStreams`s using a TLS connection defined by an SslContext.
    pub fn https_with_context(handler: H, ssl_context: SslContext) -> Server<'a, H, HttpListener> {
        Server {
            handler: handler,
            ssl: Some(SslConfig::Context(ssl_context)),
            _marker: PhantomData
        }
    }
}

impl<'a, H: Handler + 'static> Server<'a, H, HttpListener> {
    /// Binds to a socket, and starts handling connections using a task pool.
    pub fn listen_threads<T: ToSocketAddrs>(self, addr: T, threads: usize) -> ::Result<Listening> {
        let listener = try!(match self.ssl {
            Some(SslConfig::CertAndKey(cert, key)) => HttpListener::https(addr, cert, key),
            Some(SslConfig::Context(ssl_context)) => HttpListener::https_with_context(addr, ssl_context),
            None => HttpListener::http(addr)
        });
        with_listener(self.handler, listener, threads)
    }

    /// Binds to a socket and starts handling connections.
    pub fn listen<T: ToSocketAddrs>(self, addr: T) -> ::Result<Listening> {
        self.listen_threads(addr, num_cpus::get() * 5 / 4)
    }
}
impl<
'a,
H: Handler + 'static,
L: NetworkListener<Stream=S> + Send + 'static,
S: NetworkStream + Clone + Send> Server<'a, H, L> {
    /// Creates a new server that will handle `HttpStream`s.
    pub fn with_listener(self, listener: L, threads: usize) -> ::Result<Listening> {
        with_listener(self.handler, listener, threads)
    }
}

fn with_listener<H, L>(handler: H, mut listener: L, threads: usize) -> ::Result<Listening>
where H: Handler + 'static,
L: NetworkListener + Send + 'static {
    let socket = try!(listener.local_addr());

    debug!("threads = {:?}", threads);
    let pool = ListenerPool::new(listener.clone());
    let work = move |mut stream| Worker(&handler).handle_connection(&mut stream);

    let guard = thread::spawn(move || pool.accept(work, threads));

    Ok(Listening {
        _guard: Some(guard),
        socket: socket,
    })
}

struct Worker<'a, H: Handler + 'static>(&'a H);

impl<'a, H: Handler + 'static> Worker<'a, H> {

    fn handle_connection<S>(&self, mut stream: &mut S) where S: NetworkStream + Clone {
        debug!("Incoming stream");
        let addr = match stream.peer_addr() {
            Ok(addr) => addr,
            Err(e) => {
                error!("Peer Name error: {:?}", e);
                return;
            }
        };

        // FIXME: Use Type ascription
        let stream_clone: &mut NetworkStream = &mut stream.clone();
        let rdr = BufReader::new(stream_clone);
        let wrt = BufWriter::new(stream);

        self.keep_alive_loop(rdr, wrt, addr);
        debug!("keep_alive loop ending for {}", addr);
    }

    fn keep_alive_loop<W: Write>(&self, mut rdr: BufReader<&mut NetworkStream>, mut wrt: W, addr: SocketAddr) {
        let mut keep_alive = true;
        while keep_alive {
            let req = match Request::new(&mut rdr, addr) {
                Ok(req) => req,
                Err(Error::Io(ref e)) if e.kind() == ErrorKind::ConnectionAborted => {
                    trace!("tcp closed, cancelling keep-alive loop");
                    break;
                }
                Err(Error::Io(e)) => {
                    debug!("ioerror in keepalive loop = {:?}", e);
                    break;
                }
                Err(e) => {
                    //TODO: send a 400 response
                    error!("request error = {:?}", e);
                    break;
                }
            };


            if !self.handle_expect(&req, &mut wrt) {
                break;
            }

            keep_alive = http::should_keep_alive(req.version, &req.headers);
            let version = req.version;
            let mut res_headers = Headers::new();
            if !keep_alive {
                res_headers.set(Connection::close());
            }
            {
                let mut res = Response::new(&mut wrt, &mut res_headers);
                res.version = version;
                self.0.handle(req, res);
            }

            // if the request was keep-alive, we need to check that the server agrees
            // if it wasn't, then the server cannot force it to be true anyways
            if keep_alive {
                keep_alive = http::should_keep_alive(version, &res_headers);
            }

            debug!("keep_alive = {:?} for {}", keep_alive, addr);
        }
 
    }

    fn handle_expect<W: Write>(&self, req: &Request, wrt: &mut W) -> bool {
         if req.version == Http11 && req.headers.get() == Some(&Expect::Continue) {
            let status = self.0.check_continue((&req.method, &req.uri, &req.headers));
            match write!(wrt, "{} {}\r\n\r\n", Http11, status) {
                Ok(..) => (),
                Err(e) => {
                    error!("error writing 100-continue: {:?}", e);
                    return false;
                }
            }

            if status != StatusCode::Continue {
                debug!("non-100 status ({}) for Expect 100 request", status);
                return false;
            }
        }

        true
    }
}

/// A listening server, which can later be closed.
pub struct Listening {
    _guard: Option<JoinHandle<()>>,
    /// The socket addresses that the server is bound to.
    pub socket: SocketAddr,
}

impl fmt::Debug for Listening {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Listening {{ socket: {:?} }}", self.socket)
    }
}

impl Drop for Listening {
    fn drop(&mut self) {
        let _ = self._guard.take().map(|g| g.join());
    }
}

impl Listening {
    /// Stop the server from listening to its socket address.
    pub fn close(&mut self) -> ::Result<()> {
        let _ = self._guard.take();
        debug!("closing server");
        //try!(self.acceptor.close());
        Ok(())
    }
}

/// A handler that can handle incoming requests for a server.
pub trait Handler: Sync + Send {
    /// Receives a `Request`/`Response` pair, and should perform some action on them.
    ///
    /// This could reading from the request, and writing to the response.
    fn handle<'a, 'k>(&'a self, Request<'a, 'k>, Response<'a, Fresh>);

    /// Called when a Request includes a `Expect: 100-continue` header.
    ///
    /// By default, this will always immediately response with a `StatusCode::Continue`,
    /// but can be overridden with custom behavior.
    fn check_continue(&self, _: (&Method, &RequestUri, &Headers)) -> StatusCode {
        StatusCode::Continue
    }
}

impl<F> Handler for F where F: Fn(Request, Response<Fresh>), F: Sync + Send {
    fn handle<'a, 'k>(&'a self, req: Request<'a, 'k>, res: Response<'a, Fresh>) {
        self(req, res)
    }
}

#[cfg(test)]
mod tests {
    use header::Headers;
    use method::Method;
    use mock::MockStream;
    use status::StatusCode;
    use uri::RequestUri;

    use super::{Request, Response, Fresh, Handler, Worker};

    #[test]
    fn test_check_continue_default() {
        let mut mock = MockStream::with_input(b"\
            POST /upload HTTP/1.1\r\n\
            Host: example.domain\r\n\
            Expect: 100-continue\r\n\
            Content-Length: 10\r\n\
            \r\n\
            1234567890\
        ");

        fn handle(_: Request, res: Response<Fresh>) {
            res.start().unwrap().end().unwrap();
        }

        Worker(&handle).handle_connection(&mut mock);
        let cont = b"HTTP/1.1 100 Continue\r\n\r\n";
        assert_eq!(&mock.write[..cont.len()], cont);
        let res = b"HTTP/1.1 200 OK\r\n";
        assert_eq!(&mock.write[cont.len()..cont.len() + res.len()], res);
    }

    #[test]
    fn test_check_continue_reject() {
        struct Reject;
        impl Handler for Reject {
            fn handle<'a, 'k>(&'a self, _: Request<'a, 'k>, res: Response<'a, Fresh>) {
                res.start().unwrap().end().unwrap();
            }

            fn check_continue(&self, _: (&Method, &RequestUri, &Headers)) -> StatusCode {
                StatusCode::ExpectationFailed
            }
        }

        let mut mock = MockStream::with_input(b"\
            POST /upload HTTP/1.1\r\n\
            Host: example.domain\r\n\
            Expect: 100-continue\r\n\
            Content-Length: 10\r\n\
            \r\n\
            1234567890\
        ");

        Worker(&Reject).handle_connection(&mut mock);
        assert_eq!(mock.write, &b"HTTP/1.1 417 Expectation Failed\r\n\r\n"[..]);
    }
}
