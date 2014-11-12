//! HTTP Server
use std::io::{Listener, EndOfFile, BufferedReader, BufferedWriter};
use std::io::net::ip::{IpAddr, Port, SocketAddr};
use std::os;
use std::sync::{Arc, TaskPool};
use std::task::TaskBuilder;


pub use self::request::Request;
pub use self::response::Response;

pub use net::{Fresh, Streaming};

use {HttpResult};
use header::common::Connection;
use header::common::connection::{KeepAlive, Close};
use net::{NetworkListener, NetworkAcceptor, NetworkStream,
          HttpAcceptor, HttpListener, HttpStream};
use version::HttpVersion::{Http10, Http11};

pub mod request;
pub mod response;

/// A server can listen on a TCP socket.
///
/// Once listening, it will create a `Request`/`Response` pair for each
/// incoming connection, and hand them to the provided handler.
pub struct Server<L = HttpListener> {
    ip: IpAddr,
    port: Port
}

macro_rules! try_option(
    ($e:expr) => {{
        match $e {
            Some(v) => v,
            None => return None
        }
    }}
)

impl Server<HttpListener> {
    /// Creates a new server that will handle `HttpStream`s.
    pub fn http(ip: IpAddr, port: Port) -> Server {
        Server {
            ip: ip,
            port: port
        }
    }
}

impl<L: NetworkListener<S, A>, S: NetworkStream, A: NetworkAcceptor<S>> Server<L> {
    /// Binds to a socket, and starts handling connections using a task pool.
    ///
    /// This method has unbound type parameters, so can be used when you want to use
    /// something other than the provided HttpStream, HttpAcceptor, and HttpListener.
    pub fn listen_network<H, S, A, L>(self, handler: H, threads: uint) -> HttpResult<Listening<A>>
    where H: Handler,
          S: NetworkStream,
          A: NetworkAcceptor<S>,
          L: NetworkListener<S, A>, {
        debug!("binding to {}:{}", self.ip, self.port);
        let mut listener: L = try!(NetworkListener::<S, A>::bind((self.ip, self.port)));

        let socket = try!(listener.socket_name());

        let acceptor = try!(listener.listen());

        let mut captured = acceptor.clone();
        TaskBuilder::new().named("hyper acceptor").spawn(proc() {
            let handler = Arc::new(handler);
            debug!("threads = {}", threads);
            let pool = TaskPool::new(threads);
            for conn in captured.incoming() {
                match conn {
                    Ok(mut stream) => {
                        debug!("Incoming stream");
                        let handler = handler.clone();
                        pool.execute(proc() {
                            let addr = match stream.peer_name() {
                                Ok(addr) => addr,
                                Err(e) => {
                                    error!("Peer Name error: {}", e);
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
                                    Err(e) => {
                                        //TODO: send a 400 response
                                        error!("request error: {}", e);
                                        return;
                                    }
                                };

                                keep_alive = match (req.version, req.headers.get::<Connection>()) {
                                    (Http10, Some(conn)) if !conn.0.contains(&KeepAlive) => false,
                                    (Http11, Some(conn)) if conn.0.contains(&Close)  => false,
                                    _ => true
                                };
                                res.version = req.version;
                                handler.handle(req, res);
                                debug!("keep_alive = {}", keep_alive);
                            }

                        });
                    },
                    Err(ref e) if e.kind == EndOfFile => {
                        debug!("server closed");
                        break;
                    },
                    Err(e) => {
                        error!("Connection failed: {}", e);
                        continue;
                    }
                }
            }
        });

        Ok(Listening {
            acceptor: acceptor,
            socket: socket,
        })
    }

    /// Binds to a socket and starts handling connections with the specified number of tasks.
    pub fn listen_threads<H: Handler>(self, handler: H, threads: uint) -> HttpResult<Listening<HttpAcceptor>> {
        self.listen_network::<H, HttpStream, HttpAcceptor, HttpListener>(handler, threads)
    }

    /// Binds to a socket and starts handling connections.
    pub fn listen<H: Handler>(self, handler: H) -> HttpResult<Listening<HttpAcceptor>> {
        self.listen_threads(handler, os::num_cpus() * 5 / 4)
    }

}

/// A listening server, which can later be closed.
pub struct Listening<A = HttpAcceptor> {
    acceptor: A,
    /// The socket addresses that the server is bound to.
    pub socket: SocketAddr,
}

impl<A: NetworkAcceptor<S>, S: NetworkStream> Listening<A> {
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
    fn handle(&self, Request, Response<Fresh>);
}

impl Handler for fn(Request, Response<Fresh>) {
    fn handle(&self, req: Request, res: Response<Fresh>) {
        (*self)(req, res)
    }
}

