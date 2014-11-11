//! HTTP Server
use std::io::{Listener, EndOfFile};
use std::io::net::ip::{IpAddr, Port, SocketAddr};
use std::task::TaskBuilder;

use macceptor::{MoveAcceptor, MoveConnections};

pub use self::request::Request;
pub use self::response::Response;

use {HttpResult};
use net::{NetworkListener, NetworkAcceptor, NetworkStream,
          HttpAcceptor, HttpListener, HttpStream,
          Fresh};

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
    /// Binds to a socket, and starts handling connections.
    ///
    /// This method has unbound type parameters, so can be used when you want to use
    /// something other than the provided HttpStream, HttpAcceptor, and HttpListener.
    pub fn listen_network<H, S, A, L>(self, handler: H) -> HttpResult<Listening<A>>
    where H: Handler<A, S>,
          S: NetworkStream,
          A: NetworkAcceptor<S>,
          L: NetworkListener<S, A>, {
        debug!("binding to {}:{}", self.ip, self.port);
        let mut listener: L = try!(NetworkListener::<S, A>::bind((self.ip, self.port)));

        let socket = try!(listener.socket_name());

        let acceptor = try!(listener.listen());

        let captured = acceptor.clone();
        TaskBuilder::new().named("hyper acceptor").spawn(proc() {
            handler.handle(Incoming { from: captured.move_incoming() });
        });

        Ok(Listening {
            acceptor: acceptor,
            socket: socket,
        })
    }

    /// Binds to a socket and starts handling connections.
    pub fn listen<H: Handler<HttpAcceptor, HttpStream>>(self, handler: H) -> HttpResult<Listening<HttpAcceptor>> {
        self.listen_network::<H, HttpStream, HttpAcceptor, HttpListener>(handler)
    }
}

/// An iterator over incoming `Connection`s.
pub struct Incoming<A = HttpAcceptor> {
    from: MoveConnections<A>
}

impl<S: NetworkStream + 'static, A: NetworkAcceptor<S>> Iterator<Connection<S>> for Incoming<A> {
    fn next(&mut self) -> Option<Connection<S>> {
        for conn in self.from {
            match conn {
                Ok(stream) => {
                    debug!("Incoming stream");
                    return Some(Connection(stream));
                },
                Err(ref e) if e.kind == EndOfFile => return None, // server closed
                Err(e) => {
                    error!("Connection failed: {}", e);
                    continue;
                }
            }
        }
        None
    }
}

/// An incoming connection. It can be opened to receive a request/response pair.
pub struct Connection<S: Send = HttpStream>(S);

impl<S: NetworkStream + 'static> Connection<S> {
    /// Opens the incoming connection, parsing it into a Request/Response pair.
    pub fn open(self) -> HttpResult<(Request, Response<Fresh>)> {
        let stream = self.0;
        let clone = stream.clone();
        let req = try!(Request::new(stream));
        let mut res = Response::new(clone);
        res.version = req.version;
        return Ok((req, res))
    }
}

/// A listening server, which can later be closed.
pub struct Listening<A = HttpAcceptor> {
    acceptor: A,
    /// The socket addresses that the server is bound to.
    pub socket: SocketAddr,
}

impl<A: NetworkAcceptor<S>, S: NetworkStream> Listening<A> {
    /// Stop the server from listening to all of its socket addresses.
    ///
    /// If closing any of the servers acceptors fails, this function returns Err
    /// and does not close the rest of the acceptors.
    pub fn close(&mut self) -> HttpResult<()> {
        debug!("closing server");
        try!(self.acceptor.close());
        Ok(())
    }
}

/// A handler that can handle incoming requests for a server.
pub trait Handler<A: NetworkAcceptor<S>, S: NetworkStream>: Send {
    /// Receives a `Request`/`Response` pair, and should perform some action on them.
    ///
    /// This could reading from the request, and writing to the response.
    fn handle(self, Incoming<A>);
}

impl<A: NetworkAcceptor<S>, S: NetworkStream> Handler<A, S> for fn(Incoming<A>) {
    fn handle(self, incoming: Incoming<A>) {
        (self)(incoming)
    }
}

