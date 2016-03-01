//! HTTP Server
//!
//! # Server
//!
//! A `Server` is created to listen on port, parse HTTP requests, and hand
//! them off to a `Handler`.
use std::fmt;
use std::marker::PhantomData;
use std::net::{SocketAddr/*, ToSocketAddrs*/};
use std::sync::mpsc;
use std::thread;

use mio::{TryAccept, Evented, EventSet, PollOpt};
use rotor::{self, Scope};

pub use self::request::Request;
pub use self::response::Response;

use http::{self, Next};
//use net::{HttpsListener, Ssl, HttpsStream};
use net::{HttpListener, HttpStream, Transport};


mod request;
mod response;
mod message;

/// A server can listen on a TCP socket.
///
/// Once listening, it will create a `Request`/`Response` pair for each
/// incoming connection, and hand them to the provided handler.
#[derive(Debug)]
pub struct Server<T: TryAccept + Evented> {
    listener: T,
}

impl<T> Server<T> where T: TryAccept + Evented, <T as TryAccept>::Output: Transport {
    /// Creates a new server with the provided Listener.
    #[inline]
    pub fn new(listener: T) -> Server<T> {
        Server {
            listener: listener,
        }
    }
}

impl Server<HttpListener> {
    /// Creates a new HTTP server listening on the provided address.
    pub fn http(addr: &SocketAddr) -> ::Result<Server<HttpListener>> {
        use ::mio::tcp::TcpListener;
        TcpListener::bind(addr)
            .map(HttpListener)
            .map(Server::new)
            .map_err(From::from)
    }
}


/*
impl<S: Ssl> Server<HttpsStream<S::Stream>> {
    /// Creates a new server that will handle `HttpStream`s over SSL.
    ///
    /// You can use any SSL implementation, as long as implements `hyper::net::Ssl`.
    pub fn https(addr: &SocketAddr, ssl: S) -> ::Result<Server<HttpsListener<S>>> {
        HttpsListener::new(addr, ssl).map(Server::new)
    }
}
*/


//impl<T: Transport> Server<T> {
impl Server<HttpListener> {
    /// Binds to a socket and starts handling connections.
    pub fn handle<H>(self, mut factory: H) -> ::Result<Listening>
    where H: HandlerFactory<HttpStream> {
        let addr = try!(self.listener.0.local_addr());
        let (notifier_tx, notifier_rx) = mpsc::channel();
        let (shutdown_tx, shutdown_rx) = mpsc::channel();
        let listener = self.listener;
        let handle = try!(thread::Builder::new().name("hyper-server".to_owned()).spawn(move || {
            let mut loop_ = rotor::Loop::new(&rotor::Config::new()).unwrap();
            loop_.add_machine_with(move |scope| {
                rotor_try!(notifier_tx.send(scope.notifier()));
                rotor_try!(scope.register(&listener, EventSet::readable(), PollOpt::level()));
                rotor::Response::ok(ServerFsm::Listener::<HttpListener, _, H>(listener, shutdown_rx, PhantomData))
            }).unwrap();
            loop_.run(move |ctrl| {
                message::Message::new(factory.create(ctrl))
            }).unwrap();
        }));

        let notifier = notifier_rx.recv().unwrap();

        Ok(Listening {
            addr: addr,
            shutdown: (shutdown_tx, notifier),
            handle: Some(handle),
        })
    }
}

enum ServerFsm<A, F, H>
where A: TryAccept + rotor::Evented,
      A::Output: Transport,
      F: http::MessageHandlerFactory<A::Output, Output=message::Message<H::Output, A::Output>>,
      H: HandlerFactory<A::Output> {
    Listener(A, mpsc::Receiver<Shutdown>, PhantomData<F>),
    Conn(http::Conn<A::Output, message::Message<H::Output, A::Output>>)
}

impl<A, F, H> rotor::Machine for ServerFsm<A, F, H>
where A: TryAccept + rotor::Evented,
      A::Output: Transport,
      F: http::MessageHandlerFactory<A::Output, Output=message::Message<H::Output, A::Output>>,
      H: HandlerFactory<A::Output> {
    type Context = F;
    type Seed = A::Output;

    fn create(seed: Self::Seed, scope: &mut Scope<F>) -> rotor::Response<Self, rotor::Void> {
        rotor_try!(scope.register(&seed, EventSet::readable(), PollOpt::level()));
        rotor::Response::ok(ServerFsm::Conn(http::Conn::new(seed)))
    }

    fn ready(self, events: EventSet, scope: &mut Scope<F>) -> rotor::Response<Self, Self::Seed> {
        match self {
            ServerFsm::Listener(listener, rx, m) => {
                match listener.accept() {
                    Ok(Some(conn)) => {
                        rotor::Response::spawn(ServerFsm::Listener(listener, rx, m), conn)
                    },
                    Ok(None) => rotor::Response::ok(ServerFsm::Listener(listener, rx, m)),
                    Err(e) => {
                        error!("listener accept error {}", e);
                        // usually fine, just keep listening
                        rotor::Response::ok(ServerFsm::Listener(listener, rx, m))
                    }
                }
            },
            ServerFsm::Conn(conn) => {
                match conn.ready(events, scope) {
                    Some((conn, None)) => rotor::Response::ok(ServerFsm::Conn(conn)),
                    Some((conn, Some(dur))) => {
                        rotor::Response::ok(ServerFsm::Conn(conn))
                            .deadline(scope.now() + dur)
                    }
                    None => rotor::Response::done()
                }
            }
        }
    }

    fn spawned(self, _scope: &mut Scope<F>) -> rotor::Response<Self, Self::Seed> {
        match self {
            ServerFsm::Listener(listener, rx, m) => {
                match listener.accept() {
                    Ok(Some(conn)) => {
                        rotor::Response::spawn(ServerFsm::Listener(listener, rx, m), conn)
                    },
                    Ok(None) => rotor::Response::ok(ServerFsm::Listener(listener, rx, m)),
                    Err(e) => {
                        error!("listener accept error {}", e);
                        // usually fine, just keep listening
                        rotor::Response::ok(ServerFsm::Listener(listener, rx, m))
                    }
                }
            },
            sock => rotor::Response::ok(sock)
        }

    }

    fn timeout(self, scope: &mut Scope<F>) -> rotor::Response<Self, Self::Seed> {
        match self {
            ServerFsm::Listener(..) => unimplemented!(),
            ServerFsm::Conn(conn) => {
                match conn.timeout(scope) {
                    Some((conn, None)) => rotor::Response::ok(ServerFsm::Conn(conn)),
                    Some((conn, Some(dur))) => {
                        rotor::Response::ok(ServerFsm::Conn(conn))
                            .deadline(scope.now() + dur)
                    }
                    None => rotor::Response::done()
                }
            }
        }
    }

    fn wakeup(self, scope: &mut Scope<F>) -> rotor::Response<Self, Self::Seed> {
        match self {
            ServerFsm::Listener(lst, shutdown, m) => {
                if shutdown.try_recv().is_ok() {
                    let _ = scope.deregister(&lst);
                    scope.shutdown_loop();
                    rotor::Response::done()
                } else {
                    rotor::Response::ok(ServerFsm::Listener(lst, shutdown, m))
                }
            },
            ServerFsm::Conn(conn) => match conn.wakeup(scope) {
                Some((conn, None)) => rotor::Response::ok(ServerFsm::Conn(conn)),
                Some((conn, Some(dur))) => {
                    rotor::Response::ok(ServerFsm::Conn(conn))
                        .deadline(scope.now() + dur)
                }
                None => rotor::Response::done()
            }
        }
    }
}


#[derive(Debug)]
struct Shutdown;

/// A handle of the running server.
pub struct Listening {
    /// The address this server is listening on.
    pub addr: SocketAddr,
    shutdown: (mpsc::Sender<Shutdown>, rotor::Notifier),
    handle: Option<::std::thread::JoinHandle<()>>,
}

impl fmt::Debug for Listening {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Listening")
            .field("addr", &self.addr)
            .finish()
    }
}

impl Drop for Listening {
    fn drop(&mut self) {
        self.handle.take().map(|handle| {
            handle.join().unwrap();
        });
    }
}

impl Listening {
    /// Starts the Server, blocking until it is shutdown.
    pub fn forever(self) {

    }
    /// Stop the server from listening to its socket address.
    pub fn close(self) {
        debug!("closing server");
        self.shutdown.0.send(Shutdown).unwrap();
        self.shutdown.1.wakeup().unwrap();
    }
}

/// A trait to react to server events that happen for each message.
///
/// Each event handler returns it's desired `Next` action.
pub trait Handler<T: Transport>: Send + 'static {
    /// This event occurs first, triggering when a `Request` has been parsed.
    fn on_request(&mut self, request: Request) -> Next;
    /// This event occurs each time the `Request` is ready to be read from.
    fn on_request_readable(&mut self, request: &mut http::Decoder<T>) -> Next;
    /// This event occurs after the first time this handled signals `Next::write()`.
    fn on_response(&mut self, response: &mut Response) -> Next;
    /// This event occurs each time the `Response` is ready to be written to.
    fn on_response_writable(&mut self, response: &mut http::Encoder<T>) -> Next;

    /// This event occurs whenever an `Error` occurs outside of the other events.
    ///
    /// This could IO errors while waiting for events, or a timeout, etc.
    fn on_error(&mut self, err: ::Error) -> Next where Self: Sized {
        debug!("default Handler.on_error({:?})", err);
        http::Next::remove()
    }
}


/// Used to create a `Handler` when a new message is received by the server.
pub trait HandlerFactory<T: Transport>: Send + 'static {
    /// The `Handler` to use for the incoming message.
    type Output: Handler<T>;
    /// Creates the associated `Handler`.
    fn create(&mut self, ctrl: http::Control) -> Self::Output;
}

impl<F, H, T> HandlerFactory<T> for F
where F: FnMut(http::Control) -> H + Send + 'static, H: Handler<T>, T: Transport {
    type Output = H;
    fn create(&mut self, ctrl: http::Control) -> H {
        self(ctrl)
    }
}

#[cfg(test)]
mod tests {
    /*
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
            fn handle(&self, _: Request, res: Response<Fresh>) {
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
    */
}
