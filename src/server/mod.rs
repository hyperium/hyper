//! HTTP Server
use std::io::{BufReader, BufWriter};
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
use header::Connection;
use header::ConnectionOption::{Close, KeepAlive};
use net::{NetworkListener, NetworkStream, HttpListener};
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
        let work = move |stream| keep_alive_loop(stream, &handler);

        let guard = thread::scoped(move || pool.accept(work, threads));

        Ok(Listening {
            _guard: guard,
            socket: socket,
        })
    }
}


fn keep_alive_loop<'h, S, H>(mut stream: S, handler: &'h H)
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
        keep_alive = handle_connection(addr, &mut rdr, &mut wrt, handler);
        debug!("keep_alive = {:?}", keep_alive);
    }
}

fn handle_connection<'a, 'aa, 'h, S, H>(
    addr: SocketAddr,
    rdr: &'a mut BufReader<&'aa mut NetworkStream>,
    wrt: &mut BufWriter<S>,
    handler: &'h H
) -> bool where 'aa: 'a, S: NetworkStream, H: Handler {
    let mut res = Response::new(wrt);
    let req = match Request::<'a, 'aa>::new(rdr, addr) {
        Ok(req) => req,
        Err(e@HttpIoError(_)) => {
            debug!("ioerror in keepalive loop = {:?}", e);
            return false;
        }
        Err(e) => {
            //TODO: send a 400 response
            error!("request error = {:?}", e);
            return false;
        }
    };

    let keep_alive = match (req.version, req.headers.get::<Connection>()) {
        (Http10, Some(conn)) if !conn.contains(&KeepAlive) => false,
        (Http11, Some(conn)) if conn.contains(&Close)  => false,
        _ => true
    };
    res.version = req.version;
    handler.handle(req, res);
    keep_alive
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
    fn handle<'a, 'aa, 'b, 's>(&'s self, Request<'aa, 'a>, Response<'b, Fresh>);
}

impl<F> Handler for F where F: Fn(Request, Response<Fresh>), F: Sync + Send {
    fn handle<'a, 'aa, 'b, 's>(&'s self, req: Request<'a, 'aa>, res: Response<'b, Fresh>) {
        self(req, res)
    }
}
