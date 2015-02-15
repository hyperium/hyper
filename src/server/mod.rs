//! HTTP Server
use std::old_io::{Listener, BufferedReader, BufferedWriter};
use std::old_io::net::ip::{IpAddr, Port, SocketAddr};
use std::os;
use std::thread::JoinGuard;

pub use self::request::Request;
pub use self::response::Response;

pub use net::{Fresh, Streaming};

use HttpError::HttpIoError;
use {HttpResult};
use header::Connection;
use header::ConnectionOption::{Close, KeepAlive};
use net::{NetworkListener, NetworkStream, NetworkAcceptor,
          HttpAcceptor, HttpListener};
use version::HttpVersion::{Http10, Http11};

use self::acceptor::AcceptorPool;

pub mod request;
pub mod response;

mod acceptor;

/// A server can listen on a TCP socket.
///
/// Once listening, it will create a `Request`/`Response` pair for each
/// incoming connection, and hand them to the provided handler.
pub struct Server<L = HttpListener> {
    ip: IpAddr,
    port: Port,
    listener: L,
}

macro_rules! try_option(
    ($e:expr) => {{
        match $e {
            Some(v) => v,
            None => return None
        }
    }}
);

impl Server<HttpListener> {
    /// Creates a new server that will handle `HttpStream`s.
    pub fn http(ip: IpAddr, port: Port) -> Server {
        Server::with_listener(ip, port, HttpListener::Http)
    }
    /// Creates a new server that will handler `HttpStreams`s using a TLS connection.
    pub fn https(ip: IpAddr, port: Port, cert: Path, key: Path) -> Server {
        Server::with_listener(ip, port, HttpListener::Https(cert, key))
    }
}

impl<
L: NetworkListener<Acceptor=A> + Send,
A: NetworkAcceptor<Stream=S> + Send,
S: NetworkStream + Clone + Send> Server<L> {
    /// Creates a new server that will handle `HttpStream`s.
    pub fn with_listener(ip: IpAddr, port: Port, listener: L) -> Server<L> {
        Server {
            ip: ip,
            port: port,
            listener: listener,
        }
    }

    /// Binds to a socket, and starts handling connections using a task pool.
    pub fn listen_threads<H: Handler>(mut self, handler: H, threads: usize) -> HttpResult<Listening<L::Acceptor>> {
        debug!("binding to {:?}:{:?}", self.ip, self.port);
        let acceptor = try!(self.listener.listen((self.ip, self.port)));
        let socket = try!(acceptor.socket_name());

        debug!("threads = {:?}", threads);
        let pool = AcceptorPool::new(acceptor.clone());
        let work = move |stream| handle_connection(stream, &handler);

        Ok(Listening {
            _guard: pool.accept(work, threads),
            socket: socket,
            acceptor: acceptor
        })
    }

    /// Binds to a socket and starts handling connections.
    pub fn listen<H: Handler>(self, handler: H) -> HttpResult<Listening<L::Acceptor>> {
        self.listen_threads(handler, os::num_cpus() * 5 / 4)
    }

}

fn handle_connection<S, H>(mut stream: S, handler: &H)
where S: NetworkStream + Clone, H: Handler {
    debug!("Incoming stream");
    let addr = match stream.peer_name() {
        Ok(addr) => addr,
        Err(e) => {
            error!("Peer Name error: {:?}", e);
            return;
        }
    };

    let mut rdr = BufferedReader::new(stream.clone());
    let mut wrt = BufferedWriter::new(stream);

    let mut keep_alive = true;
    while keep_alive {
        let mut res = Response::new(&mut wrt);
        let req = match Request::new(&mut rdr, addr) {
            Ok(req) => req,
            Err(e@HttpIoError(_)) => {
                debug!("ioerror in keepalive loop = {:?}", e);
                return;
            }
            Err(e) => {
                //TODO: send a 400 response
                error!("request error = {:?}", e);
                return;
            }
        };

        keep_alive = match (req.version, req.headers.get::<Connection>()) {
            (Http10, Some(conn)) if !conn.contains(&KeepAlive) => false,
            (Http11, Some(conn)) if conn.contains(&Close)  => false,
            _ => true
        };
        res.version = req.version;
        handler.handle(req, res);
        debug!("keep_alive = {:?}", keep_alive);
    }
}

/// A listening server, which can later be closed.
pub struct Listening<A = HttpAcceptor> {
    acceptor: A,
    _guard: JoinGuard<'static, ()>,
    /// The socket addresses that the server is bound to.
    pub socket: SocketAddr,
}

impl<A: NetworkAcceptor> Listening<A> {
    /// Stop the server from listening to its socket address.
    pub fn close(&mut self) -> HttpResult<()> {
        debug!("closing server");
        try!(self.acceptor.close());
        Ok(())
    }
}

/// A handler that can handle incoming requests for a server.
pub trait Handler: Sync + Send {
    /// Receives a `Request`/`Response` pair, and should perform some action on them.
    ///
    /// This could reading from the request, and writing to the response.
    fn handle<'a>(&'a self, Request<'a>, Response<'a, Fresh>);
}

impl<F> Handler for F where F: Fn(Request, Response<Fresh>), F: Sync + Send {
    fn handle(&self, req: Request, res: Response<Fresh>) {
        (*self)(req, res)
    }
}
