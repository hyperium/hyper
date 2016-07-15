//! HTTP Server
//!
//! A `Server` is created to listen on a port, parse HTTP requests, and hand
//! them off to a `Handler`.
use std::fmt;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use rotor::mio::{EventSet, PollOpt};
use rotor::{self, Scope};

pub use self::request::Request;
pub use self::response::Response;

use http::{self, Next};

pub use net::{Accept, HttpListener, HttpsListener};
use net::{SslServer, Transport};


mod request;
mod response;
mod message;

/// A configured `Server` ready to run.
pub struct ServerLoop<A, H> where A: Accept, H: HandlerFactory<A::Output> {
    inner: Option<(rotor::Loop<ServerFsm<A, H>>, Context<H>)>,
}

impl<A: Accept, H: HandlerFactory<A::Output>> fmt::Debug for ServerLoop<A, H> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad("ServerLoop")
    }
}

/// A Server that can accept incoming network requests.
#[derive(Debug)]
pub struct Server<T: Accept> {
    listener: T,
    keep_alive: bool,
    idle_timeout: Duration,
    max_sockets: usize,
}

impl<T> Server<T> where T: Accept, T::Output: Transport {
    /// Creates a new server with the provided Listener.
    #[inline]
    pub fn new(listener: T) -> Server<T> {
        Server {
            listener: listener,
            keep_alive: true,
            idle_timeout: Duration::from_secs(10),
            max_sockets: 4096,
        }
    }

    /// Enables or disables HTTP keep-alive.
    ///
    /// Default is true.
    pub fn keep_alive(mut self, val: bool) -> Server<T> {
        self.keep_alive = val;
        self
    }

    /// Sets how long an idle connection will be kept before closing.
    ///
    /// Default is 10 seconds.
    pub fn idle_timeout(mut self, val: Duration) -> Server<T> {
        self.idle_timeout = val;
        self
    }

    /// Sets the maximum open sockets for this Server.
    ///
    /// Default is 4096, but most servers can handle much more than this.
    pub fn max_sockets(mut self, val: usize) -> Server<T> {
        self.max_sockets = val;
        self
    }
}

impl Server<HttpListener> { //<H: HandlerFactory<<HttpListener as Accept>::Output>> Server<HttpListener, H> {
    /// Creates a new HTTP server config listening on the provided address.
    pub fn http(addr: &SocketAddr) -> ::Result<Server<HttpListener>> {
        use ::rotor::mio::tcp::TcpListener;
        TcpListener::bind(addr)
            .map(HttpListener)
            .map(Server::new)
            .map_err(From::from)
    }
}


impl<S: SslServer> Server<HttpsListener<S>> {
    /// Creates a new server config that will handle `HttpStream`s over SSL.
    ///
    /// You can use any SSL implementation, as long as it implements `hyper::net::Ssl`.
    pub fn https(addr: &SocketAddr, ssl: S) -> ::Result<Server<HttpsListener<S>>> {
        HttpsListener::new(addr, ssl)
            .map(Server::new)
            .map_err(From::from)
    }
}


impl<A: Accept> Server<A> where A::Output: Transport  {
    /// Binds to a socket and starts handling connections.
    pub fn handle<H>(self, factory: H) -> ::Result<(Listening, ServerLoop<A, H>)>
    where H: HandlerFactory<A::Output> {
        let addr = try!(self.listener.local_addr());
        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_rx = shutdown.clone();

        let mut config = rotor::Config::new();
        config.slab_capacity(self.max_sockets);
        config.mio().notify_capacity(self.max_sockets);
        let keep_alive = self.keep_alive;
        let idle_timeout = self.idle_timeout;
        let mut loop_ = rotor::Loop::new(&config).unwrap();
        let mut notifier = None;
        {
            let notifier = &mut notifier;
            loop_.add_machine_with(move |scope| {
                *notifier = Some(scope.notifier());
                rotor_try!(scope.register(&self.listener, EventSet::readable(), PollOpt::level()));
                rotor::Response::ok(ServerFsm::Listener::<A, H>(self.listener, shutdown_rx))
            }).unwrap();
        }
        let notifier = notifier.expect("loop.add_machine failed");

        let listening = Listening {
            addr: addr,
            shutdown: (shutdown, notifier),
        };
        let server = ServerLoop {
            inner: Some((loop_, Context {
                factory: factory,
                idle_timeout: idle_timeout,
                keep_alive: keep_alive,
            }))
        };
        Ok((listening, server))
    }
}


impl<A: Accept, H: HandlerFactory<A::Output>> ServerLoop<A, H> {
    /// Runs the server forever in this loop.
    ///
    /// This will block the current thread.
    pub fn run(self) {
        // drop will take care of it.
    }
}

impl<A: Accept, H: HandlerFactory<A::Output>> Drop for ServerLoop<A, H> {
    fn drop(&mut self) {
        self.inner.take().map(|(loop_, ctx)| {
            let _ = loop_.run(ctx);
        });
    }
}

struct Context<F> {
    factory: F,
    idle_timeout: Option<Duration>,
    keep_alive: bool,
}

impl<F: HandlerFactory<T>, T: Transport> http::MessageHandlerFactory<(), T> for Context<F> {
    type Output = message::Message<F::Output, T>;

    fn create(&mut self, seed: http::Seed<()>) -> Option<Self::Output> {
        Some(message::Message::new(self.factory.create(seed.control())))
    }

    fn keep_alive_interest(&self) -> Next {
        if let Some(dur) = self.idle_timeout {
            Next::read().timeout(dur)
        } else {
            Next::read()
        }
    }
}

enum ServerFsm<A, H>
where A: Accept,
      A::Output: Transport,
      H: HandlerFactory<A::Output> {
    Listener(A, Arc<AtomicBool>),
    Conn(http::Conn<(), A::Output, message::Message<H::Output, A::Output>>)
}

impl<A, H> rotor::Machine for ServerFsm<A, H>
where A: Accept,
      A::Output: Transport,
      H: HandlerFactory<A::Output> {
    type Context = Context<H>;
    type Seed = A::Output;

    fn create(seed: Self::Seed, scope: &mut Scope<Self::Context>) -> rotor::Response<Self, rotor::Void> {
        rotor_try!(scope.register(&seed, EventSet::readable(), PollOpt::level()));
        rotor::Response::ok(
            ServerFsm::Conn(
                http::Conn::new((), seed, Next::read(), scope.notifier())
                    .keep_alive(scope.keep_alive)
            )
        )
    }

    fn ready(self, events: EventSet, scope: &mut Scope<Self::Context>) -> rotor::Response<Self, Self::Seed> {
        match self {
            ServerFsm::Listener(listener, rx) => {
                match listener.accept() {
                    Ok(Some(conn)) => {
                        rotor::Response::spawn(ServerFsm::Listener(listener, rx), conn)
                    },
                    Ok(None) => rotor::Response::ok(ServerFsm::Listener(listener, rx)),
                    Err(e) => {
                        error!("listener accept error {}", e);
                        // usually fine, just keep listening
                        rotor::Response::ok(ServerFsm::Listener(listener, rx))
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

    fn spawned(self, _scope: &mut Scope<Self::Context>) -> rotor::Response<Self, Self::Seed> {
        match self {
            ServerFsm::Listener(listener, rx) => {
                match listener.accept() {
                    Ok(Some(conn)) => {
                        rotor::Response::spawn(ServerFsm::Listener(listener, rx), conn)
                    },
                    Ok(None) => rotor::Response::ok(ServerFsm::Listener(listener, rx)),
                    Err(e) => {
                        error!("listener accept error {}", e);
                        // usually fine, just keep listening
                        rotor::Response::ok(ServerFsm::Listener(listener, rx))
                    }
                }
            },
            sock => rotor::Response::ok(sock)
        }

    }

    fn timeout(self, scope: &mut Scope<Self::Context>) -> rotor::Response<Self, Self::Seed> {
        match self {
            ServerFsm::Listener(..) => unreachable!("Listener cannot timeout"),
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

    fn wakeup(self, scope: &mut Scope<Self::Context>) -> rotor::Response<Self, Self::Seed> {
        match self {
            ServerFsm::Listener(lst, shutdown) => {
                if shutdown.load(Ordering::Acquire) {
                    let _ = scope.deregister(&lst);
                    scope.shutdown_loop();
                    rotor::Response::done()
                } else {
                    rotor::Response::ok(ServerFsm::Listener(lst, shutdown))
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

/// A handle of the running server.
pub struct Listening {
    addr: SocketAddr,
    shutdown: (Arc<AtomicBool>, rotor::Notifier),
}

impl fmt::Debug for Listening {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Listening")
            .field("addr", &self.addr)
            .field("closed", &self.shutdown.0.load(Ordering::Relaxed))
            .finish()
    }
}

impl fmt::Display for Listening {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.addr, f)
    }
}

impl Listening {
    /// The address this server is listening on.
    pub fn addr(&self) -> &SocketAddr {
        &self.addr
    }

    /// Stop the server from listening to its socket address.
    pub fn close(self) {
        debug!("closing server {}", self);
        self.shutdown.0.store(true, Ordering::Release);
        self.shutdown.1.wakeup().unwrap();
    }
}

/// A trait to react to server events that happen for each message.
///
/// Each event handler returns its desired `Next` action.
pub trait Handler<T: Transport> {
    /// This event occurs first, triggering when a `Request` has been parsed.
    fn on_request(&mut self, request: Request<T>) -> Next;
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

    /// This event occurs when this Handler has requested to remove the Transport.
    fn on_remove(self, _transport: T) where Self: Sized {
        debug!("default Handler.on_remove");
    }
}


/// Used to create a `Handler` when a new message is received by the server.
pub trait HandlerFactory<T: Transport> {
    /// The `Handler` to use for the incoming message.
    type Output: Handler<T>;
    /// Creates the associated `Handler`.
    fn create(&mut self, ctrl: http::Control) -> Self::Output;
}

impl<F, H, T> HandlerFactory<T> for F
where F: FnMut(http::Control) -> H, H: Handler<T>, T: Transport {
    type Output = H;
    fn create(&mut self, ctrl: http::Control) -> H {
        self(ctrl)
    }
}

#[cfg(test)]
mod tests {

}
