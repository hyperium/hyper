//! # Server
use std::io::net::tcp::TcpListener;
use std::io::{Acceptor, Listener, IoResult};
use std::io::net::ip::{IpAddr, Port};

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
    pub fn listen<H: Handler>(&self, mut handler: H) {
        let listener = match TcpListener::bind(self.ip.to_string().as_slice(), self.port) {
            Ok(listener) => listener,
            Err(err) => fail!("Listen failed: {}", err)
        };
        let mut acceptor = listener.listen();

        for conn in acceptor.incoming() {
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
                    match handler.handle(req, res) {
                        Ok(..) => debug!("Stream handled"),
                        Err(e) => {
                            error!("Error from handler: {}", e)
                            //TODO try to send a status code
                        }
                    }
                },
                Err(e) => {
                    error!("Connection failed: {}", e);
                }
            }
        }
    }

}

/// A handler that can handle incoming requests for a server.
pub trait Handler {
    /// Receives a `Request`/`Response` pair, and should perform some action on them.
    ///
    /// This could reading from the request, and writing to the response.
    fn handle(&mut self, req: Request, res: Response) -> IoResult<()>;
}

impl<'a> Handler for |Request, Response|: 'a -> IoResult<()> {
    fn handle(&mut self, req: Request, res: Response) -> IoResult<()> {
        (*self)(req, res)
    }
}
