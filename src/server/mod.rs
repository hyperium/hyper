//! HTTP Server
use std::io::net::tcp::{TcpListener, TcpAcceptor};
use std::io::{Acceptor, Listener, IoResult, EndOfFile, IncomingConnections};
use std::io::net::ip::{IpAddr, Port, SocketAddr};

pub use self::request::Request;
pub use self::response::Response;

pub mod request;
pub mod response;

/// A server can listen on a TCP socket.
///
/// Once listening, it will create a `Request`/`Response` pair for each
/// incoming connection, and hand them to the provided handler.
pub struct Server {
    ip: IpAddr,
    port: Port
}


impl Server {

    /// Creates a server to be used for `http` conenctions.
    pub fn http(ip: IpAddr, port: Port) -> Server {
        Server {
            ip: ip,
            port: port
        }
    }

    /// Binds to a socket, and starts handling connections.
    pub fn listen<H: Handler + 'static>(self, handler: H) -> IoResult<Listening> {
        let mut listener = try!(TcpListener::bind(self.ip.to_string().as_slice(), self.port));
        let socket = try!(listener.socket_name());
        let acceptor = try!(listener.listen());
        let worker = acceptor.clone();

        spawn(proc() {
            let mut acceptor = worker;
            handler.handle(Incoming { from: acceptor.incoming() });
        });

        Ok(Listening {
            acceptor: acceptor,
            socket_addr: socket,
        })
    }

}

/// An iterator over incoming connections, represented as pairs of
/// hyper Requests and Responses.
pub struct Incoming<'a> {
    from: IncomingConnections<'a, TcpAcceptor>
}

impl<'a> Iterator<(Request, Response)> for Incoming<'a> {
    fn next(&mut self) -> Option<(Request, Response)> {
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
pub struct Listening {
    acceptor: TcpAcceptor,
    /// The socket address that the server is bound to.
    pub socket_addr: SocketAddr,
}

impl Listening {
    /// Stop the server from listening to it's socket address.
    pub fn close(mut self) -> IoResult<()> {
        debug!("closing server");
        self.acceptor.close_accept()
    }
}

/// A handler that can handle incoming requests for a server.
pub trait Handler: Send {
    /// Receives a `Request`/`Response` pair, and should perform some action on them.
    ///
    /// This could reading from the request, and writing to the response.
    fn handle(self, Incoming);
}

impl Handler for fn(Incoming) {
    fn handle(self, incoming: Incoming) {
        (self)(incoming)
    }
}

