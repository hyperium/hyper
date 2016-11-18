//! HTTP Server
//!
//! A `Server` is created to listen on a port, parse HTTP requests, and hand
//! them off to a `Handler`.
use std::cell::RefCell;
use std::fmt;
use std::io;
use std::marker::PhantomData;
use std::net::SocketAddr;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use futures::{Future, Async, Map};
use futures::stream::{Stream};

use tokio::reactor::{Core, Handle};
use tokio_proto::server::listen as tokio_listen;
use tokio_proto::pipeline;
use tokio_proto::{Message, Body};
pub use tokio_service::{NewService, Service};

pub use self::request::Request;
pub use self::response::Response;

//use self::conn::Conn;

use http;

pub use net::{Accept, HttpListener};
use net::{HttpStream, Transport};
/*
pub use net::{Accept, HttpListener, HttpsListener};
use net::{SslServer, Transport};
*/

//mod conn;
mod request;
mod response;

/// A Server that can accept incoming network requests.
#[derive(Debug)]
pub struct Server<A> {
    //listeners: Vec<A>,
    _marker: PhantomData<A>,
    addr: SocketAddr,
    keep_alive: bool,
    idle_timeout: Option<Duration>,
    max_sockets: usize,
}

impl<A: Accept> Server<A> {
    /*
    /// Creates a new Server from one or more Listeners.
    ///
    /// Panics if listeners is an empty iterator.
    pub fn new<I: IntoIterator<Item = A>>(listeners: I) -> Server<A> {
        let listeners = listeners.into_iter().collect();

        Server {
            listeners: listeners,
            keep_alive: true,
            idle_timeout: Some(Duration::from_secs(10)),
            max_sockets: 4096,
        }
    }
    */

    /// Enables or disables HTTP keep-alive.
    ///
    /// Default is true.
    pub fn keep_alive(mut self, val: bool) -> Server<A> {
        self.keep_alive = val;
        self
    }

    /// Sets how long an idle connection will be kept before closing.
    ///
    /// Default is 10 seconds.
    pub fn idle_timeout(mut self, val: Option<Duration>) -> Server<A> {
        self.idle_timeout = val;
        self
    }

    /// Sets the maximum open sockets for this Server.
    ///
    /// Default is 4096, but most servers can handle much more than this.
    pub fn max_sockets(mut self, val: usize) -> Server<A> {
        self.max_sockets = val;
        self
    }
}

impl Server<HttpListener> { //<H: HandlerFactory<<HttpListener as Accept>::Output>> Server<HttpListener, H> {
    /// Creates a new HTTP server config listening on the provided address.
    pub fn http(addr: &SocketAddr) -> ::Result<Server<HttpListener>> {
        Ok(Server {
            _marker: PhantomData,
            addr: addr.clone(),
            keep_alive: true,
            idle_timeout: Some(Duration::from_secs(10)),
            max_sockets: 4096,
        })
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


impl/*<A: Accept>*/ Server<HttpListener> {
    /// Binds to a socket and starts handling connections.
    pub fn handle<H>(mut self, factory: H, handle: &Handle) -> ::Result<SocketAddr>
    where H: NewService<Request=Request, Response=Response, Error=::Error> + Send + 'static {
        let addr = self.addr;
        let h = try!(tokio_listen(&handle, addr, move |sock| {
            let service = HttpService { inner: try!(factory.new_service()) };
            let conn = http::Conn::<_, http::ServerTransaction>::new(sock);
            Ok(pipeline::Server::new(service, conn))
        }));
        Ok(h.local_addr().clone())
    }

    pub fn standalone<H>(mut self, factory: H) -> ::Result<(Listening, ServerLoop)>
    where H: NewService<Request=Request, Response=Response, Error=::Error> + Send + 'static {
        let mut core = try!(Core::new());
        let handle = core.handle();
        let addr = try!(self.handle(factory, &handle));
        let (shutdown_tx, shutdown_rx) = try!(::tokio::channel::channel(&handle));
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
    inner: Option<(Core, ::tokio::channel::Receiver<()>)>,
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
    }
}

impl Drop for ServerLoop {
    fn drop(&mut self) {
        self.inner.take().map(|(mut loop_, work)| {
            let _ = loop_.run(work.into_future());
            debug!("server closed");
        });
    }
}

/// A handle of the running server.
pub struct Listening {
    addr: SocketAddr,
    shutdown: ::tokio::channel::Sender<()>,
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
        let _ = self.shutdown.send(());
    }
}

struct HttpService<T> {
    inner: T,
}

fn map_response_to_message(res: Response) -> Message<ResponseHead, Body<http::Chunk, ::Error>> {
    let (head, body) = response::split(res);
    if let Some(body) = body {
        Message::WithBody(head, body)
    } else {
        Message::WithoutBody(head)
    }
}

type ResponseHead = http::MessageHead<::StatusCode>;

impl<T> Service for HttpService<T>
    where T: Service<Request=Request, Response=Response, Error=::Error>,
{
    type Request = Message<http::RequestHead, Body<http::Chunk, ::Error>>;
    type Response = Message<ResponseHead, Body<http::Chunk, ::Error>>;
    type Error = ::Error;
    type Future = Map<T::Future, fn(Response) -> Message<ResponseHead, Body<http::Chunk, ::Error>>>;

    fn call(&self, message: Self::Request) -> Self::Future {
        let req = match message {
            Message::WithoutBody(head) => Request::new(head, None),
            Message::WithBody(head, body) => Request::new(head, Some(body)),
        };
        self.inner.call(req).map(map_response_to_message)
    }

}
