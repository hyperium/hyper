//! HTTP Server
use std::io::{Acceptor, Listener, IoResult, EndOfFile, IncomingConnections};
use std::io::net::ip::{IpAddr, Port, SocketAddr};

pub use self::request::Request;
pub use self::response::{Response, Fresh, Streaming};

use net::{NetworkListener, NetworkAcceptor, NetworkStream};
use net::{HttpListener, HttpAcceptor};

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

    /// Creates a server that can listen for and handle `NetworkStreams`.
    pub fn new(ip: IpAddr, port: Port) -> Server<L> {
        Server {
            ip: ip,
            port: port
        }
    }


    /// Binds to a socket, and starts handling connections.
    pub fn listen<H: Handler<A, S> + 'static>(self, handler: H) -> IoResult<Listening<A>> {
        let mut listener: L = try!(NetworkListener::bind(self.ip.to_string().as_slice(), self.port));
        let socket = try!(listener.socket_name());
        let acceptor = try!(listener.listen());
        let mut worker = acceptor.clone();

        spawn(proc() {
            handler.handle(Incoming { from: worker.incoming() });
        });

        Ok(Listening {
            acceptor: acceptor,
            socket_addr: socket,
        })
    }

}

/// An iterator over incoming connections, represented as pairs of
/// hyper Requests and Responses.
pub struct Incoming<'a, A: 'a = HttpAcceptor> {
    from: IncomingConnections<'a, A>
}

impl<'a, A: NetworkAcceptor<S>, S: NetworkStream> Iterator<(Request<S>, Response<Fresh, S>)> for Incoming<'a, A> {
    fn next(&mut self) -> Option<(Request<S>, Response<Fresh, S>)> {
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
    acceptor: A,
    /// The socket address that the server is bound to.
    pub socket_addr: SocketAddr,
}

impl<A: NetworkAcceptor<S>, S: NetworkStream> Listening<A> {
    /// Stop the server from listening to it's socket address.
    pub fn close(mut self) -> IoResult<()> {
        debug!("closing server");
        self.acceptor.close()
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

