//! HTTP Server
//!
//! A `Server` is created to listen on a port, parse HTTP requests, and hand
//! them off to a `Service`.
use std::fmt;
use std::io;
use std::net::{SocketAddr, TcpListener as StdTcpListener};

use futures::{Future, Map};
use futures::stream::{Stream};
use futures::sync::oneshot;

use tokio::io::Io;
use tokio::net::TcpListener;
use tokio::reactor::{Core, Handle};
use tokio_proto::BindServer;
use tokio_proto::streaming::Message;
use tokio_proto::streaming::pipeline::ServerProto;
pub use tokio_service::{NewService, Service};

pub use self::accept::Accept;
pub use self::request::Request;
pub use self::response::Response;

use http;

mod request;
mod response;

type HttpIncoming = ::tokio::net::Incoming;

/// A Server that can accept incoming network requests.
#[derive(Debug)]
pub struct Server<A> {
    accepter: A,
    addr: SocketAddr,
    keep_alive: bool,
    //idle_timeout: Option<Duration>,
    //max_sockets: usize,
}

impl<A: Accept> Server<A> {
    /// Creates a new Server from a Stream of Ios.
    ///
    /// The addr is the socket address the accepter is listening on.
    pub fn new(accepter: A, addr: SocketAddr) -> Server<A> {
        Server {
            accepter: accepter,
            addr: addr,
            keep_alive: true,
            //idle_timeout: Some(Duration::from_secs(75)),
            //max_sockets: 4096,
        }
    }

    /// Enables or disables HTTP keep-alive.
    ///
    /// Default is true.
    pub fn keep_alive(mut self, val: bool) -> Server<A> {
        self.keep_alive = val;
        self
    }

    /*
    /// Sets how long an idle connection will be kept before closing.
    ///
    /// Default is 75 seconds.
    pub fn idle_timeout(mut self, val: Option<Duration>) -> Server<A> {
        self.idle_timeout = val;
        self
    }
    */

    /*
    /// Sets the maximum open sockets for this Server.
    ///
    /// Default is 4096, but most servers can handle much more than this.
    pub fn max_sockets(mut self, val: usize) -> Server<A> {
        self.max_sockets = val;
        self
    }
    */
}

impl Server<HttpIncoming> {
    /// Creates a new HTTP server config listening on the provided address.
    pub fn http(addr: &SocketAddr, handle: &Handle) -> ::Result<Server<HttpIncoming>> {
        let listener = try!(StdTcpListener::bind(addr));
        let addr = try!(listener.local_addr());
        let listener = try!(TcpListener::from_listener(listener, &addr, handle));
        Ok(Server::new(listener.incoming(), addr))
    }
}


/*
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
*/


impl<A: Accept> Server<A> {
    /// Binds to a socket and starts handling connections.
    pub fn handle<H>(self, factory: H, handle: &Handle) -> ::Result<SocketAddr>
    where H: NewService<Request=Request, Response=Response, Error=::Error> + 'static {
        let binder = HttpServer {
            keep_alive: self.keep_alive,
        };
        let inner_handle = handle.clone();
        handle.spawn(self.accepter.accept().for_each(move |(socket, remote_addr)| {
            let service = HttpService {
                inner: try!(factory.new_service()),
                remote_addr: remote_addr,
            };
            binder.bind_server(&inner_handle, socket, service);
            Ok(())
        }).map_err(|e| {
            error!("listener io error: {:?}", e);
            ()
        }));

        Ok(self.addr)
    }
}

impl Server<()> {
    /// Create a server that owns its event loop.
    ///
    /// The returned `ServerLoop` can be used to run the loop forever in the
    /// thread. The returned `Listening` can be sent to another thread, and
    /// used to shutdown the `ServerLoop`.
    pub fn standalone<F>(closure: F) -> ::Result<(Listening, ServerLoop)>
    where F: FnOnce(&Handle) -> ::Result<SocketAddr> {
        let core = try!(Core::new());
        let handle = core.handle();
        let addr = try!(closure(&handle));
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        Ok((
            Listening {
                addr: addr,
                shutdown: shutdown_tx,
            },
             ServerLoop {
                inner: Some((core, shutdown_rx)),
            }
        ))

    }
}

/// A configured `Server` ready to run.
pub struct ServerLoop {
    inner: Option<(Core, oneshot::Receiver<()>)>,
}

impl fmt::Debug for ServerLoop {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad("ServerLoop")
    }
}

impl ServerLoop {
    /// Runs the server forever in this loop.
    ///
    /// This will block the current thread.
    pub fn run(self) {
        // drop will take care of it.
        trace!("ServerLoop::run()");
    }
}

impl Drop for ServerLoop {
    fn drop(&mut self) {
        self.inner.take().map(|(mut loop_, shutdown)| {
            debug!("ServerLoop::drop running");
            let _ = loop_.run(shutdown.or_else(|_dropped| ::futures::future::empty::<(), oneshot::Canceled>()));
            debug!("Server closed");
        });
    }
}

/// A handle of the running server.
pub struct Listening {
    addr: SocketAddr,
    shutdown: ::futures::sync::oneshot::Sender<()>,
}

impl fmt::Debug for Listening {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Listening")
            .field("addr", &self.addr)
            .finish()
    }
}

impl fmt::Display for Listening {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.addr, f)
    }
}

impl Listening {
    /// The addresses this server is listening on.
    pub fn addr(&self) -> &SocketAddr {
        &self.addr
    }

    /// Stop the server from listening to its socket address.
    pub fn close(self) {
        debug!("closing server {}", self);
        self.shutdown.complete(());
    }
}

struct HttpServer {
    keep_alive: bool,
}

impl<T: Io + 'static> ServerProto<T> for HttpServer {
    type Request = http::RequestHead;
    type RequestBody = http::Chunk;
    type Response = ResponseHead;
    type ResponseBody = http::Chunk;
    type Error = ::Error;
    type Transport = http::Conn<T, http::ServerTransaction>;
    type BindTransport = io::Result<http::Conn<T, http::ServerTransaction>>;

    fn bind_transport(&self, io: T) -> Self::BindTransport {
        let ka = if self.keep_alive {
            http::KA::Busy
        } else {
            http::KA::Disabled
        };
        Ok(http::Conn::new(io, ka))
    }
}

struct HttpService<T> {
    inner: T,
    remote_addr: SocketAddr,
}

fn map_response_to_message(res: Response) -> Message<ResponseHead, http::TokioBody> {
    let (head, body) = response::split(res);
    if let Some(body) = body {
        Message::WithBody(head, body.into())
    } else {
        Message::WithoutBody(head)
    }
}

type ResponseHead = http::MessageHead<::StatusCode>;

impl<T> Service for HttpService<T>
    where T: Service<Request=Request, Response=Response, Error=::Error>,
{
    type Request = Message<http::RequestHead, http::TokioBody>;
    type Response = Message<ResponseHead, http::TokioBody>;
    type Error = ::Error;
    type Future = Map<T::Future, fn(Response) -> Message<ResponseHead, http::TokioBody>>;

    fn call(&self, message: Self::Request) -> Self::Future {
        let (head, body) = match message {
            Message::WithoutBody(head) => (head, http::Body::empty()),
            Message::WithBody(head, body) => (head, body.into()),
        };
        let req = request::new(self.remote_addr, head, body);
        self.inner.call(req).map(map_response_to_message)
    }
}

//private so the `Acceptor` type can stay internal
mod accept {
    use std::io;
    use std::net::SocketAddr;
    use futures::{Stream, Poll};
    use tokio::io::Io;

    /// An Acceptor is an incoming Stream of Io.
    ///
    /// This trait is not implemented directly, and only exists to make the
    /// intent clearer. A `Stream<Item=(Io, SocketAddr), Error=io::Error>`
    /// should be implemented instead.
    pub trait Accept: Stream<Error=io::Error> {
        #[doc(hidden)]
        type Output: Io + 'static;
        #[doc(hidden)]
        type Stream: Stream<Item=(Self::Output, SocketAddr), Error=io::Error> + 'static;

        #[doc(hidden)]
        fn accept(self) -> Accepter<Self::Stream, Self::Output>
            where Self: Sized;
    }

    #[allow(missing_debug_implementations)]
    pub struct Accepter<T: Stream<Item=(I, SocketAddr), Error=io::Error> + 'static, I: Io + 'static>(T, ::std::marker::PhantomData<I>);

    impl<T, I> Stream for Accepter<T, I>
    where T: Stream<Item=(I, SocketAddr), Error=io::Error>,
          I: Io + 'static,
    {
        type Item = T::Item;
        type Error = io::Error;

        #[inline]
        fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
            self.0.poll()
        }
    }

    impl<T, I> Accept for T
    where T: Stream<Item=(I, SocketAddr), Error=io::Error> + 'static,
          I: Io + 'static,
    {
        type Output = I;
        type Stream = T;

        fn accept(self) -> Accepter<Self, I> {
            Accepter(self, ::std::marker::PhantomData)
        }
    }
}
