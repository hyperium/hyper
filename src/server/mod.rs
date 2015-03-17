//! HTTP Server
use std::io::{BufReader, BufWriter, Write};
use std::marker::PhantomData;
use std::net::{IpAddr, SocketAddr};
use std::path::Path;
use std::thread::{self, JoinGuard};

use num_cpus;

pub use self::request::Request;
pub use self::response::Response;

pub use net::{Fresh, Streaming};

use HttpError::HttpIoError;
use {HttpResult};
use header::{Headers, Connection, Expect};
use header::ConnectionOption::{Close, KeepAlive};
use method::Method;
use net::{NetworkListener, NetworkStream, HttpListener};
use status::StatusCode;
use uri::RequestUri;
use version::HttpVersion::{Http10, Http11};

use self::listener::ListenerPool;

pub mod request;
pub mod response;

mod listener;

/// A server can listen on a TCP socket.
///
/// Once listening, it will create a `Request`/`Response` pair for each
/// incoming connection, and hand them to the provided handler.
pub struct Server<'a, H: Handler, L = HttpListener> {
    handler: H,
    ssl: Option<(&'a Path, &'a Path)>,
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
            ssl: Some((cert, key)),
            _marker: PhantomData
        }
    }
}

impl<'a, H: Handler + 'static> Server<'a, H, HttpListener> {
    /// Binds to a socket, and starts handling connections using a task pool.
    pub fn listen_threads(self, ip: IpAddr, port: u16, threads: usize) -> HttpResult<Listening> {
        let addr = &(ip, port);
        let listener = try!(match self.ssl {
            Some((cert, key)) => HttpListener::https(addr, cert, key),
            None => HttpListener::http(addr)
        });
        self.with_listener(listener, threads)
    }

    /// Binds to a socket and starts handling connections.
    pub fn listen(self, ip: IpAddr, port: u16) -> HttpResult<Listening> {
        self.listen_threads(ip, port, num_cpus::get() * 5 / 4)
    }
}
impl<
'a,
H: Handler + 'static,
L: NetworkListener<Stream=S> + Send + 'static,
S: NetworkStream + Clone + Send> Server<'a, H, L> {
    /// Creates a new server that will handle `HttpStream`s.
    pub fn with_listener(self, mut listener: L, threads: usize) -> HttpResult<Listening> {
        let socket = try!(listener.socket_addr());
        let handler = self.handler;

        debug!("threads = {:?}", threads);
        let pool = ListenerPool::new(listener.clone());
        let work = move |mut stream| handle_connection(&mut stream, &handler);

        let guard = thread::scoped(move || pool.accept(work, threads));

        Ok(Listening {
            _guard: guard,
            socket: socket,
        })
    }
}


fn handle_connection<'h, S, H>(mut stream: &mut S, handler: &'h H)
where S: NetworkStream + Clone, H: Handler {
    debug!("Incoming stream");
    let addr = match stream.peer_addr() {
        Ok(addr) => addr,
        Err(e) => {
            error!("Peer Name error: {:?}", e);
            return;
        }
    };

    let mut stream_clone = stream.clone();
    let mut rdr = BufReader::new(&mut stream_clone as &mut NetworkStream);
    let mut wrt = BufWriter::new(stream);

    let mut keep_alive = true;
    while keep_alive {
        let req = match Request::new(&mut rdr, addr) {
            Ok(req) => req,
            Err(e@HttpIoError(_)) => {
                debug!("ioerror in keepalive loop = {:?}", e);
                break;
            }
            Err(e) => {
                //TODO: send a 400 response
                error!("request error = {:?}", e);
                break;
            }
        };

        if req.version == Http11 && req.headers.get() == Some(&Expect::Continue) {
            let status = handler.check_continue((&req.method, &req.uri, &req.headers));
            match write!(&mut wrt, "{} {}\r\n\r\n", Http11, status) {
                Ok(..) => (),
                Err(e) => {
                    error!("error writing 100-continue: {:?}", e);
                    break;
                }
            }

            if status != StatusCode::Continue {
                debug!("non-100 status ({}) for Expect 100 request", status);
                break;
            }
        }

        keep_alive = match (req.version, req.headers.get::<Connection>()) {
            (Http10, Some(conn)) if !conn.contains(&KeepAlive) => false,
            (Http11, Some(conn)) if conn.contains(&Close)  => false,
            _ => true
        };
        let mut res = Response::new(&mut wrt);
        res.version = req.version;
        handler.handle(req, res);
        debug!("keep_alive = {:?}", keep_alive);
    }
}

/// A listening server, which can later be closed.
pub struct Listening {
    _guard: JoinGuard<'static, ()>,
    /// The socket addresses that the server is bound to.
    pub socket: SocketAddr,
}

impl Listening {
    /// Stop the server from listening to its socket address.
    pub fn close(&mut self) -> HttpResult<()> {
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

    use super::{Request, Response, Fresh, Handler, handle_connection};

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

        handle_connection(&mut mock, &handle);
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

        handle_connection(&mut mock, &Reject);
        assert_eq!(mock.write, b"HTTP/1.1 417 Expectation Failed\r\n\r\n");
    }
}
