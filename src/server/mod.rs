//! HTTP Server
//!
//! # Server
//!
//! A `Server` is created to listen on port, parse HTTP requests, and hand
//! them off to a `Handler`. By default, the Server will listen across multiple
//! threads, but that can be configured to a single thread if preferred.
//!
//! # Handling requests
//!
//! You must pass a `Handler` to the Server that will handle requests. There is
//! a default implementation for `fn`s and closures, allowing you pass one of
//! those easily.
//!
//!
//! ```no_run
//! use hyper::server::{Server, Request, Response};
//!
//! fn hello(req: Request, res: Response) {
//!     // handle things here
//! }
//!
//! Server::http("0.0.0.0:0").unwrap().handle(hello).unwrap();
//! ```
//!
//! As with any trait, you can also define a struct and implement `Handler`
//! directly on your own type, and pass that to the `Server` instead.
//!
//! ```no_run
//! use std::sync::Mutex;
//! use std::sync::mpsc::{channel, Sender};
//! use hyper::server::{Handler, Server, Request, Response};
//!
//! struct SenderHandler {
//!     sender: Mutex<Sender<&'static str>>
//! }
//!
//! impl Handler for SenderHandler {
//!     fn handle(&self, req: Request, res: Response) {
//!         self.sender.lock().unwrap().send("start").unwrap();
//!     }
//! }
//!
//!
//! let (tx, rx) = channel();
//! Server::http("0.0.0.0:0").unwrap().handle(SenderHandler {
//!     sender: Mutex::new(tx)
//! }).unwrap();
//! ```
//!
//! Since the `Server` will be listening on multiple threads, the `Handler`
//! must implement `Sync`: any mutable state must be synchronized.
//!
//! ```no_run
//! use std::sync::atomic::{AtomicUsize, Ordering};
//! use hyper::server::{Server, Request, Response};
//!
//! let counter = AtomicUsize::new(0);
//! Server::http("0.0.0.0:0").unwrap().handle(move |req: Request, res: Response| {
//!     counter.fetch_add(1, Ordering::Relaxed);
//! }).unwrap();
//! ```
//!
//! # The `Request` and `Response` pair
//!
//! A `Handler` receives a pair of arguments, a `Request` and a `Response`. The
//! `Request` includes access to the `method`, `uri`, and `headers` of the
//! incoming HTTP request. It also implements `std::io::Read`, in order to
//! read any body, such as with `POST` or `PUT` messages.
//!
//! Likewise, the `Response` includes ways to set the `status` and `headers`,
//! and implements `std::io::Write` to allow writing the response body.
//!
//! ```no_run
//! use std::io;
//! use hyper::server::{Server, Request, Response};
//! use hyper::status::StatusCode;
//!
//! Server::http("0.0.0.0:0").unwrap().handle(|mut req: Request, mut res: Response| {
//!     match req.method {
//!         hyper::Post => {
//!             io::copy(&mut req, &mut res.start().unwrap()).unwrap();
//!         },
//!         _ => *res.status_mut() = StatusCode::MethodNotAllowed
//!     }
//! }).unwrap();
//! ```
//!
//! ## An aside: Write Status
//!
//! The `Response` uses a phantom type parameter to determine its write status.
//! What does that mean? In short, it ensures you never write a body before
//! adding all headers, and never add a header after writing some of the body.
//!
//! This is often done in most implementations by include a boolean property
//! on the response, such as `headers_written`, checking that each time the
//! body has something to write, so as to make sure the headers are sent once,
//! and only once. But this has 2 downsides:
//!
//! 1. You are typically never notified that your late header is doing nothing.
//! 2. There's a runtime cost to checking on every write.
//!
//! Instead, hyper handles this statically, or at compile-time. A
//! `Response<Fresh>` includes a `headers_mut()` method, allowing you add more
//! headers. It also does not implement `Write`, so you can't accidentally
//! write early. Once the "head" of the response is correct, you can "send" it
//! out by calling `start` on the `Response<Fresh>`. This will return a new
//! `Response<Streaming>` object, that no longer has `headers_mut()`, but does
//! implement `Write`.
use std::fmt;
use std::io::{self, ErrorKind, BufWriter, Write};
use std::net::{SocketAddr, ToSocketAddrs};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use num_cpus;

pub use self::request::Request;
pub use self::response::Response;

pub use net::{Fresh, Streaming};

use Error;
use buffer::BufReader;
use header::{Headers, Expect, Connection};
use http;
use method::Method;
use net::{NetworkListener, NetworkStream, HttpListener, HttpsListener, SslServer};
use status::StatusCode;
use uri::RequestUri;
use version::HttpVersion::Http11;

use self::listener::ListenerPool;

pub mod request;
pub mod response;

mod listener;

/// A server can listen on a TCP socket.
///
/// Once listening, it will create a `Request`/`Response` pair for each
/// incoming connection, and hand them to the provided handler.
#[derive(Debug)]
pub struct Server<L = HttpListener> {
    listener: L,
    timeouts: Timeouts,
}

#[derive(Clone, Copy, Debug)]
struct Timeouts {
    read: Option<Duration>,
    keep_alive: Option<Duration>,
}

impl Default for Timeouts {
    fn default() -> Timeouts {
        Timeouts {
            read: None,
            keep_alive: Some(Duration::from_secs(5))
        }
    }
}

macro_rules! try_option(
    ($e:expr) => {{
        match $e {
            Some(v) => v,
            None => return None
        }
    }}
);

impl<L: NetworkListener> Server<L> {
    /// Creates a new server with the provided handler.
    #[inline]
    pub fn new(listener: L) -> Server<L> {
        Server {
            listener: listener,
            timeouts: Timeouts::default()
        }
    }

    /// Controls keep-alive for this server.
    ///
    /// The timeout duration passed will be used to determine how long
    /// to keep the connection alive before dropping it.
    ///
    /// Passing `None` will disable keep-alive.
    ///
    /// Default is enabled with a 5 second timeout.
    #[inline]
    pub fn keep_alive(&mut self, timeout: Option<Duration>) {
        self.timeouts.keep_alive = timeout;
    }

    /// Sets the read timeout for all Request reads.
    pub fn set_read_timeout(&mut self, dur: Option<Duration>) {
        self.listener.set_read_timeout(dur);
        self.timeouts.read = dur;
    }

    /// Sets the write timeout for all Response writes.
    pub fn set_write_timeout(&mut self, dur: Option<Duration>) {
        self.listener.set_write_timeout(dur);
    }

    /// Get the address that the server is listening on.
    pub fn local_addr(&mut self) -> io::Result<SocketAddr> {
        self.listener.local_addr()
    }
}

impl Server<HttpListener> {
    /// Creates a new server that will handle `HttpStream`s.
    pub fn http<To: ToSocketAddrs>(addr: To) -> ::Result<Server<HttpListener>> {
        HttpListener::new(addr).map(Server::new)
    }
}

impl<S: SslServer + Clone + Send> Server<HttpsListener<S>> {
    /// Creates a new server that will handle `HttpStream`s over SSL.
    ///
    /// You can use any SSL implementation, as long as implements `hyper::net::Ssl`.
    pub fn https<A: ToSocketAddrs>(addr: A, ssl: S) -> ::Result<Server<HttpsListener<S>>> {
        HttpsListener::new(addr, ssl).map(Server::new)
    }
}

impl<L: NetworkListener + Send + 'static> Server<L> {
    /// Binds to a socket and starts handling connections.
    pub fn handle<H: Handler + 'static>(self, handler: H) -> ::Result<Listening> {
        self.handle_threads(handler, num_cpus::get() * 5 / 4)
    }

    /// Binds to a socket and starts handling connections with the provided
    /// number of threads.
    pub fn handle_threads<H: Handler + 'static>(self, handler: H,
            threads: usize) -> ::Result<Listening> {
        handle(self, handler, threads)
    }
}

fn handle<H, L>(mut server: Server<L>, handler: H, threads: usize) -> ::Result<Listening>
where H: Handler + 'static, L: NetworkListener + Send + 'static {
    let socket = try!(server.listener.local_addr());

    debug!("threads = {:?}", threads);
    let pool = ListenerPool::new(server.listener);
    let worker = Worker::new(handler, server.timeouts);
    let work = move |mut stream| worker.handle_connection(&mut stream);

    let guard = thread::spawn(move || pool.accept(work, threads));

    Ok(Listening {
        _guard: Some(guard),
        socket: socket,
    })
}

struct Worker<H: Handler + 'static> {
    handler: H,
    timeouts: Timeouts,
}

impl<H: Handler + 'static> Worker<H> {
    fn new(handler: H, timeouts: Timeouts) -> Worker<H> {
        Worker {
            handler: handler,
            timeouts: timeouts,
        }
    }

    fn handle_connection<S>(&self, mut stream: &mut S) where S: NetworkStream + Clone {
        debug!("Incoming stream");

        self.handler.on_connection_start();

        let addr = match stream.peer_addr() {
            Ok(addr) => addr,
            Err(e) => {
                error!("Peer Name error: {:?}", e);
                return;
            }
        };

        // FIXME: Use Type ascription
        let stream_clone: &mut NetworkStream = &mut stream.clone();
        let mut rdr = BufReader::new(stream_clone);
        let mut wrt = BufWriter::new(stream);

        while self.keep_alive_loop(&mut rdr, &mut wrt, addr) {
            if let Err(e) = self.set_read_timeout(*rdr.get_ref(), self.timeouts.keep_alive) {
                error!("set_read_timeout keep_alive {:?}", e);
                break;
            }
        }

        self.handler.on_connection_end();

        debug!("keep_alive loop ending for {}", addr);
    }

    fn set_read_timeout(&self, s: &NetworkStream, timeout: Option<Duration>) -> io::Result<()> {
        s.set_read_timeout(timeout)
    }

    fn keep_alive_loop<W: Write>(&self, mut rdr: &mut BufReader<&mut NetworkStream>,
            wrt: &mut W, addr: SocketAddr) -> bool {
        let req = match Request::new(rdr, addr) {
            Ok(req) => req,
            Err(Error::Io(ref e)) if e.kind() == ErrorKind::ConnectionAborted => {
                trace!("tcp closed, cancelling keep-alive loop");
                return false;
            }
            Err(Error::Io(e)) => {
                debug!("ioerror in keepalive loop = {:?}", e);
                return false;
            }
            Err(e) => {
                //TODO: send a 400 response
                error!("request error = {:?}", e);
                return false;
            }
        };

        if !self.handle_expect(&req, wrt) {
            return false;
        }

        if let Err(e) = req.set_read_timeout(self.timeouts.read) {
            error!("set_read_timeout {:?}", e);
            return false;
        }

        let mut keep_alive = self.timeouts.keep_alive.is_some() &&
            http::should_keep_alive(req.version, &req.headers);
        let version = req.version;
        let mut res_headers = Headers::new();
        if !keep_alive {
            res_headers.set(Connection::close());
        }
        {
            let mut res = Response::new(wrt, &mut res_headers);
            res.version = version;
            self.handler.handle(req, res);
        }

        // if the request was keep-alive, we need to check that the server agrees
        // if it wasn't, then the server cannot force it to be true anyways
        if keep_alive {
            keep_alive = http::should_keep_alive(version, &res_headers);
        }

        debug!("keep_alive = {:?} for {}", keep_alive, addr);
        keep_alive
    }

    fn handle_expect<W: Write>(&self, req: &Request, wrt: &mut W) -> bool {
         if req.version == Http11 && req.headers.get() == Some(&Expect::Continue) {
            let status = self.handler.check_continue((&req.method, &req.uri, &req.headers));
            match write!(wrt, "{} {}\r\n\r\n", Http11, status).and_then(|_| wrt.flush()) {
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
    /// Warning: This function doesn't work. The server remains listening after you called
    /// it. See https://github.com/hyperium/hyper/issues/338 for more details.
    ///
    /// Stop the server from listening to its socket address.
    pub fn close(&mut self) -> ::Result<()> {
        let _ = self._guard.take();
        debug!("closing server");
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

    /// This is run after a connection is received, on a per-connection basis (not a
    /// per-request basis, as a connection with keep-alive may handle multiple
    /// requests)
    fn on_connection_start(&self) { }

    /// This is run before a connection is closed, on a per-connection basis (not a
    /// per-request basis, as a connection with keep-alive may handle multiple
    /// requests)
    fn on_connection_end(&self) { }
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

        Worker::new(handle, Default::default()).handle_connection(&mut mock);
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

        Worker::new(Reject, Default::default()).handle_connection(&mut mock);
        assert_eq!(mock.write, &b"HTTP/1.1 417 Expectation Failed\r\n\r\n"[..]);
    }
}
