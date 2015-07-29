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
//! out by calling `start` on the `Request<Fresh>`. This will return a new
//! `Request<Streaming>` object, that no longer has `headers_mut()`, but does
//! implement `Write`.
use std::fmt;
use std::io::{self, ErrorKind, BufWriter, Write};
use std::net::{SocketAddr, ToSocketAddrs};
use std::thread::{self, JoinHandle};

#[cfg(feature = "timeouts")]
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
use net::{NetworkListener, NetworkStream, HttpListener, HttpsListener, Ssl};
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
    _timeouts: Timeouts,
}

#[cfg(feature = "timeouts")]
#[derive(Clone, Copy, Default, Debug)]
struct Timeouts {
    read: Option<Duration>,
    write: Option<Duration>,
}

#[cfg(not(feature = "timeouts"))]
#[derive(Clone, Copy, Default, Debug)]
struct Timeouts;

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
            _timeouts: Timeouts::default(),
        }
    }

    #[cfg(feature = "timeouts")]
    pub fn set_read_timeout(&mut self, dur: Option<Duration>) {
        self._timeouts.read = dur;
    }

    #[cfg(feature = "timeouts")]
    pub fn set_write_timeout(&mut self, dur: Option<Duration>) {
        self._timeouts.write = dur;
    }


}

impl Server<HttpListener> {
    /// Creates a new server that will handle `HttpStream`s.
    pub fn http<To: ToSocketAddrs>(addr: To) -> ::Result<Server<HttpListener>> {
        HttpListener::new(addr).map(Server::new)
    }
}

impl<S: Ssl + Clone + Send> Server<HttpsListener<S>> {
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
where H: Handler + 'static,
L: NetworkListener + Send + 'static {
    let socket = try!(server.listener.local_addr());

    debug!("threads = {:?}", threads);
    let pool = ListenerPool::new(server.listener);
    let worker = Worker::new(handler, server._timeouts);
    let work = move |mut stream| worker.handle_connection(&mut stream);

    let guard = thread::spawn(move || pool.accept(work, threads));

    Ok(Listening {
        _guard: Some(guard),
        socket: socket,
    })
}

struct Worker<H: Handler + 'static> {
    handler: H,
    _timeouts: Timeouts,
}

impl<H: Handler + 'static> Worker<H> {

    fn new(handler: H, timeouts: Timeouts) -> Worker<H> {
        Worker {
            handler: handler,
            _timeouts: timeouts,
        }
    }

    fn handle_connection<S>(&self, mut stream: &mut S) where S: NetworkStream + Clone {
        debug!("Incoming stream");

        if let Err(e) = self.set_timeouts(stream) {
            error!("set_timeouts error: {:?}", e);
            return;
        }

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

    #[cfg(not(feature = "timeouts"))]
    fn set_timeouts<S>(&self, _: &mut S) -> io::Result<()> where S: NetworkStream {
        Ok(())
    }

    #[cfg(feature = "timeouts")]
    fn set_timeouts<S>(&self, s: &mut S) -> io::Result<()> where S: NetworkStream {
        try!(s.set_read_timeout(self._timeouts.read));
        s.set_write_timeout(self._timeouts.write)
    }

    fn keep_alive_loop<W: Write>(&self, mut rdr: BufReader<&mut NetworkStream>,
            mut wrt: W, addr: SocketAddr) {
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
                self.handler.handle(req, res);
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
            let status = self.handler.check_continue((&req.method, &req.uri, &req.headers));
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
