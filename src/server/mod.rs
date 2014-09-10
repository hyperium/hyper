//! HTTP Server
use std::io::{Listener, IoResult, EndOfFile};
use std::io::net::ip::{IpAddr, Port, SocketAddr};

use intertwine::{Intertwine, Intertwined};
use macceptor::MoveAcceptor;

pub use self::request::Request;
pub use self::response::{Response, Fresh, Streaming};

use net::{NetworkListener, NetworkAcceptor, NetworkStream, HttpAcceptor, HttpListener, HttpStream};

use {HttpResult};

pub mod request;
pub mod response;

/// A server can listen on a TCP socket.
///
/// Once listening, it will create a `Request`/`Response` pair for each
/// incoming connection, and hand them to the provided handler.
pub struct Server<L = HttpListener> {
    pairs: Vec<(IpAddr, Port)>
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
        Server { pairs: vec![(ip, port)] }
    }

    /// Creates a server that can listen to many (ip, port) pairs.
    pub fn many(pairs: Vec<(IpAddr, Port)>) -> Server {
        Server { pairs: pairs }
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
        let mut acceptors = Vec::new();
        let mut sockets = Vec::new();
        for (ip, port) in self.pairs.move_iter() {
            let mut listener: L = try_io!(NetworkListener::<S, A>::bind(ip.to_string().as_slice(), port));

            sockets.push(try_io!(listener.socket_name()));

            let acceptor = try_io!(listener.listen());
            acceptors.push(acceptor.clone());
        }

        let connections = acceptors.clone()
            .move_iter()
            .map(|acceptor| acceptor.move_incoming())
            .intertwine();

        spawn(proc() {
            handler.handle(Incoming { from: connections });
        });

        Ok(Listening {
            acceptors: acceptors,
            sockets: sockets,
        })
    }

    /// Binds to a socket and starts handling connections.
    pub fn listen<H: Handler<HttpAcceptor, HttpStream>>(self, handler: H) -> HttpResult<Listening<HttpAcceptor>> {
        self.listen_network::<H, HttpStream, HttpAcceptor, HttpListener>(handler)
    }
}

/// An iterator over incoming connections, represented as pairs of
/// hyper Requests and Responses.
pub struct Incoming<S: Send = HttpStream> {
    from: Intertwined<IoResult<S>>
}

impl<S: NetworkStream + 'static> Iterator<(Request, Response<Fresh>)> for Incoming<S> {
    fn next(&mut self) -> Option<(Request, Response<Fresh>)> {
        for conn in self.from {
            match conn {
                Ok(stream) => {
                    debug!("Incoming stream");
                    let clone = stream.clone();
                    let req = match Request::new(stream) {
                        Ok(r) => r,
                        Err(err) => {
                            error!("creating Request: {}", err);
                            continue;
                        }
                    };
                    let mut res = Response::new(clone);
                    res.version = req.version;
                    return Some((req, res))
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

/// A listening server, which can later be closed.
pub struct Listening<A = HttpAcceptor> {
    acceptors: Vec<A>,
    /// The socket addresses that the server is bound to.
    pub sockets: Vec<SocketAddr>,
}

impl<A: NetworkAcceptor<S>, S: NetworkStream> Listening<A> {
    /// Stop the server from listening to all of its socket addresses.
    ///
    /// If closing any of the servers acceptors fails, this function returns Err
    /// and does not close the rest of the acceptors.
    pub fn close(&mut self) -> HttpResult<()> {
        debug!("closing server");
        for acceptor in self.acceptors.mut_iter() {
            try_io!(acceptor.close());
        }
        Ok(())
    }
}

/// A handler that can handle incoming requests for a server.
pub trait Handler<A: NetworkAcceptor<S>, S: NetworkStream>: Send {
    /// Receives a `Request`/`Response` pair, and should perform some action on them.
    ///
    /// This could reading from the request, and writing to the response.
    fn handle(self, Incoming<S>);
}

impl<A: NetworkAcceptor<S>, S: NetworkStream> Handler<A, S> for fn(Incoming<S>) {
    fn handle(self, incoming: Incoming<S>) {
        (self)(incoming)
    }
}

