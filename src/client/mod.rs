//! HTTP Client

use std::cell::RefCell;
use std::fmt;
use std::io;
use std::marker::PhantomData;
use std::rc::Rc;
use std::time::Duration;

use futures::{future, Poll, Async, Future, Stream};
use futures::unsync::oneshot;
#[cfg(feature = "compat")]
use http;
use tokio_io::{AsyncRead, AsyncWrite};
use tokio::reactor::Handle;
use tokio_proto::BindClient;
use tokio_proto::streaming::Message;
use tokio_proto::streaming::pipeline::ClientProto;
use tokio_proto::util::client_proxy::ClientProxy;
pub use tokio_service::Service;

use header::{Headers, Host};
use proto::{self, RequestHead, TokioBody};
use proto::response;
use proto::request;
use method::Method;
use self::pool::{Pool, Pooled};
use uri::{self, Uri};
use version::HttpVersion;

pub use proto::response::Response;
pub use proto::request::Request;
pub use self::connect::{HttpConnector, Connect};

mod connect;
mod dns;
mod pool;
#[cfg(feature = "compat")]
mod compat_impl;
#[cfg(feature = "compat")]
pub mod compat;

/// A Client to make outgoing HTTP requests.
// If the Connector is clone, then the Client can be clone easily.
pub struct Client<C, B = proto::Body> {
    connector: C,
    handle: Handle,
    pool: Dispatch<B>,
}

impl Client<HttpConnector, proto::Body> {
    /// Create a new Client with the default config.
    #[inline]
    pub fn new(handle: &Handle) -> Client<HttpConnector, proto::Body> {
        Config::default().build(handle)
    }
}

impl Client<HttpConnector, proto::Body> {
    /// Configure a Client.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # extern crate hyper;
    /// # extern crate tokio_core;
    ///
    /// # fn main() {
    /// # let core = tokio_core::reactor::Core::new().unwrap();
    /// # let handle = core.handle();
    /// let client = hyper::Client::configure()
    ///     .keep_alive(true)
    ///     .build(&handle);
    /// # drop(client);
    /// # }
    /// ```
    #[inline]
    pub fn configure() -> Config<UseDefaultConnector, proto::Body> {
        Config::default()
    }
}

impl<C, B> Client<C, B> {
    /// Return a reference to a handle to the event loop this Client is associated with.
    #[inline]
    pub fn handle(&self) -> &Handle {
        &self.handle
    }

    /// Create a new client with a specific connector.
    #[inline]
    fn configured(config: Config<C, B>, handle: &Handle) -> Client<C, B> {
        Client {
            connector: config.connector,
            handle: handle.clone(),
            pool: if config.no_proto {
                Dispatch::Hyper(Pool::new(config.keep_alive, config.keep_alive_timeout))
            } else {
                Dispatch::Proto(Pool::new(config.keep_alive, config.keep_alive_timeout))
            }
        }
    }
}

impl<C, B> Client<C, B>
where C: Connect,
      B: Stream<Error=::Error> + 'static,
      B::Item: AsRef<[u8]>,
{
    /// Send a GET Request using this Client.
    #[inline]
    pub fn get(&self, url: Uri) -> FutureResponse {
        self.request(Request::new(Method::Get, url))
    }

    /// Send a constructed Request using this Client.
    #[inline]
    pub fn request(&self, req: Request<B>) -> FutureResponse {
        self.call(req)
    }

    /// Send an `http::Request` using this Client.
    #[inline]
    #[cfg(feature = "compat")]
    pub fn request_compat(&self, req: http::Request<B>) -> compat::CompatFutureResponse {
        self::compat_impl::future(self.call(req.into()))
    }

    /// Convert into a client accepting `http::Request`.
    #[cfg(feature = "compat")]
    pub fn into_compat(self) -> compat::CompatClient<C, B> {
        self::compat_impl::client(self)
    }
}

/// A `Future` that will resolve to an HTTP Response.
#[must_use = "futures do nothing unless polled"]
pub struct FutureResponse(Box<Future<Item=Response, Error=::Error> + 'static>);

impl fmt::Debug for FutureResponse {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad("Future<Response>")
    }
}

impl Future for FutureResponse {
    type Item = Response;
    type Error = ::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        self.0.poll()
    }
}

impl<C, B> Service for Client<C, B>
where C: Connect,
      B: Stream<Error=::Error> + 'static,
      B::Item: AsRef<[u8]>,
{
    type Request = Request<B>;
    type Response = Response;
    type Error = ::Error;
    type Future = FutureResponse;

    fn call(&self, req: Self::Request) -> Self::Future {
        match req.version() {
            HttpVersion::Http10 |
            HttpVersion::Http11 => (),
            other => {
                error!("Request has unsupported version \"{}\"", other);
                return FutureResponse(Box::new(future::err(::Error::Version)));
            }
        }

        let url = req.uri().clone();
        let domain = match uri::scheme_and_authority(&url) {
            Some(uri) => uri,
            None => {
                return FutureResponse(Box::new(future::err(::Error::Io(
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "invalid URI for Client Request"
                    )
                ))));
            }
        };
        let host = Host::new(domain.host().expect("authority implies host").to_owned(), domain.port());
        let (mut head, body) = request::split(req);
        let mut headers = Headers::new();
        headers.set(host);
        headers.extend(head.headers.iter());
        head.headers = headers;

        match self.pool {
            Dispatch::Proto(ref pool) => {
                trace!("proto_dispatch");
                let checkout = pool.checkout(domain.as_ref());
                let connect = {
                    let handle = self.handle.clone();
                    let pool = pool.clone();
                    let pool_key = Rc::new(domain.to_string());
                    self.connector.connect(url)
                        .map(move |io| {
                            let (tx, rx) = oneshot::channel();
                            let client = HttpClient {
                                client_rx: RefCell::new(Some(rx)),
                            }.bind_client(&handle, io);
                            let pooled = pool.pooled(pool_key, client);
                            drop(tx.send(pooled.clone()));
                            pooled
                        })
                };

                let race = checkout.select(connect)
                    .map(|(client, _work)| client)
                    .map_err(|(e, _work)| {
                        // the Pool Checkout cannot error, so the only error
                        // is from the Connector
                        // XXX: should wait on the Checkout? Problem is
                        // that if the connector is failing, it may be that we
                        // never had a pooled stream at all
                        e.into()
                    });
                let resp = race.and_then(move |client| {
                    let msg = match body {
                        Some(body) => {
                            Message::WithBody(head, body.into())
                        },
                        None => Message::WithoutBody(head),
                    };
                    client.call(msg)
                });
                FutureResponse(Box::new(resp.map(|msg| {
                    match msg {
                        Message::WithoutBody(head) => response::from_wire(head, None),
                        Message::WithBody(head, body) => response::from_wire(head, Some(body.into())),
                    }
                })))
            },
            Dispatch::Hyper(ref pool) => {
                trace!("no_proto dispatch");
                use futures::Sink;
                use futures::sync::{mpsc, oneshot};

                let checkout = pool.checkout(domain.as_ref());
                let connect = {
                    let handle = self.handle.clone();
                    let pool = pool.clone();
                    let pool_key = Rc::new(domain.to_string());
                    self.connector.connect(url)
                        .map(move |io| {
                            let (tx, rx) = mpsc::channel(1);
                            let pooled = pool.pooled(pool_key, RefCell::new(tx));
                            let conn = proto::Conn::<_, _, proto::ClientTransaction, _>::new(io, pooled.clone());
                            let dispatch = proto::dispatch::Dispatcher::new(proto::dispatch::Client::new(rx), conn);
                            handle.spawn(dispatch.map_err(|err| error!("no_proto error: {}", err)));
                            pooled
                        })
                };

                let race = checkout.select(connect)
                    .map(|(client, _work)| client)
                    .map_err(|(e, _work)| {
                        // the Pool Checkout cannot error, so the only error
                        // is from the Connector
                        // XXX: should wait on the Checkout? Problem is
                        // that if the connector is failing, it may be that we
                        // never had a pooled stream at all
                        e.into()
                    });

                let resp = race.and_then(move |client| {
                    let (callback, rx) = oneshot::channel();
                    client.borrow_mut().start_send((head, body, callback)).unwrap();
                    rx.then(|res| {
                        match res {
                            Ok(Ok(res)) => Ok(res),
                            Ok(Err(err)) => Err(err),
                            Err(_) => panic!("dispatch dropped without returning error"),
                        }
                    })
                });

                FutureResponse(Box::new(resp))

            }
        }
    }

}

impl<C: Clone, B> Clone for Client<C, B> {
    fn clone(&self) -> Client<C, B> {
        Client {
            connector: self.connector.clone(),
            handle: self.handle.clone(),
            pool: match self.pool {
                Dispatch::Proto(ref pool) => Dispatch::Proto(pool.clone()),
                Dispatch::Hyper(ref pool) => Dispatch::Hyper(pool.clone()),
            }
        }
    }
}

impl<C, B> fmt::Debug for Client<C, B> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad("Client")
    }
}

type ProtoClient<B> = ClientProxy<Message<RequestHead, B>, Message<proto::ResponseHead, TokioBody>, ::Error>;
type HyperClient<B> = RefCell<::futures::sync::mpsc::Sender<(RequestHead, Option<B>, ::futures::sync::oneshot::Sender<::Result<::Response>>)>>;

enum Dispatch<B> {
    Proto(Pool<ProtoClient<B>>),
    Hyper(Pool<HyperClient<B>>),
}

struct HttpClient<B> {
    client_rx: RefCell<Option<oneshot::Receiver<Pooled<ProtoClient<B>>>>>,
}

impl<T, B> ClientProto<T> for HttpClient<B>
where T: AsyncRead + AsyncWrite + 'static,
      B: Stream<Error=::Error> + 'static,
      B::Item: AsRef<[u8]>,
{
    type Request = proto::RequestHead;
    type RequestBody = B::Item;
    type Response = proto::ResponseHead;
    type ResponseBody = proto::Chunk;
    type Error = ::Error;
    type Transport = proto::Conn<T, B::Item, proto::ClientTransaction, Pooled<ProtoClient<B>>>;
    type BindTransport = BindingClient<T, B>;

    fn bind_transport(&self, io: T) -> Self::BindTransport {
        BindingClient {
            rx: self.client_rx.borrow_mut().take().expect("client_rx was lost"),
            io: Some(io),
        }
    }
}

struct BindingClient<T, B> {
    rx: oneshot::Receiver<Pooled<ProtoClient<B>>>,
    io: Option<T>,
}

impl<T, B> Future for BindingClient<T, B>
where T: AsyncRead + AsyncWrite + 'static,
      B: Stream<Error=::Error>,
      B::Item: AsRef<[u8]>,
{
    type Item = proto::Conn<T, B::Item, proto::ClientTransaction, Pooled<ProtoClient<B>>>;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        match self.rx.poll() {
            Ok(Async::Ready(client)) => Ok(Async::Ready(
                    proto::Conn::new(self.io.take().expect("binding client io lost"), client)
            )),
            Ok(Async::NotReady) => Ok(Async::NotReady),
            Err(_canceled) => unreachable!(),
        }
    }
}

/// Configuration for a Client
pub struct Config<C, B> {
    _body_type: PhantomData<B>,
    //connect_timeout: Duration,
    connector: C,
    keep_alive: bool,
    keep_alive_timeout: Option<Duration>,
    //TODO: make use of max_idle config
    max_idle: usize,
    no_proto: bool,
}

/// Phantom type used to signal that `Config` should create a `HttpConnector`.
#[derive(Debug, Clone, Copy)]
pub struct UseDefaultConnector(());

impl Default for Config<UseDefaultConnector, proto::Body> {
    fn default() -> Config<UseDefaultConnector, proto::Body> {
        Config {
            _body_type: PhantomData::<proto::Body>,
            //connect_timeout: Duration::from_secs(10),
            connector: UseDefaultConnector(()),
            keep_alive: true,
            keep_alive_timeout: Some(Duration::from_secs(90)),
            max_idle: 5,
            no_proto: false,
        }
    }
}

impl<C, B> Config<C, B> {
    /// Set the body stream to be used by the `Client`.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use hyper::client::Config;
    /// let cfg = Config::default()
    ///     .body::<hyper::Body>();
    /// # drop(cfg);
    #[inline]
    pub fn body<BB>(self) -> Config<C, BB> {
        Config {
            _body_type: PhantomData::<BB>,
            //connect_timeout: self.connect_timeout,
            connector: self.connector,
            keep_alive: self.keep_alive,
            keep_alive_timeout: self.keep_alive_timeout,
            max_idle: self.max_idle,
            no_proto: self.no_proto,
        }
    }

    /// Set the `Connect` type to be used.
    #[inline]
    pub fn connector<CC>(self, val: CC) -> Config<CC, B> {
        Config {
            _body_type: self._body_type,
            //connect_timeout: self.connect_timeout,
            connector: val,
            keep_alive: self.keep_alive,
            keep_alive_timeout: self.keep_alive_timeout,
            max_idle: self.max_idle,
            no_proto: self.no_proto,
        }
    }

    /// Enable or disable keep-alive mechanics.
    ///
    /// Default is enabled.
    #[inline]
    pub fn keep_alive(mut self, val: bool) -> Config<C, B> {
        self.keep_alive = val;
        self
    }

    /// Set an optional timeout for idle sockets being kept-alive.
    ///
    /// Pass `None` to disable timeout.
    ///
    /// Default is 90 seconds.
    #[inline]
    pub fn keep_alive_timeout(mut self, val: Option<Duration>) -> Config<C, B> {
        self.keep_alive_timeout = val;
        self
    }

    /*
    /// Set the timeout for connecting to a URL.
    ///
    /// Default is 10 seconds.
    #[inline]
    pub fn connect_timeout(mut self, val: Duration) -> Config<C, B> {
        self.connect_timeout = val;
        self
    }
    */

    /// Disable tokio-proto internal usage.
    #[inline]
    pub fn no_proto(mut self) -> Config<C, B> {
        self.no_proto = true;
        self
    }
}

impl<C, B> Config<C, B>
where C: Connect,
      B: Stream<Error=::Error>,
      B::Item: AsRef<[u8]>,
{
    /// Construct the Client with this configuration.
    #[inline]
    pub fn build(self, handle: &Handle) -> Client<C, B> {
        Client::configured(self, handle)
    }
}

impl<B> Config<UseDefaultConnector, B>
where B: Stream<Error=::Error>,
      B::Item: AsRef<[u8]>,
{
    /// Construct the Client with this configuration.
    #[inline]
    pub fn build(self, handle: &Handle) -> Client<HttpConnector, B> {
        self.connector(HttpConnector::new(4, handle)).build(handle)
    }
}

impl<C, B> fmt::Debug for Config<C, B> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Config")
            .field("keep_alive", &self.keep_alive)
            .field("keep_alive_timeout", &self.keep_alive_timeout)
            .field("max_idle", &self.max_idle)
            .finish()
    }
}

impl<C: Clone, B> Clone for Config<C, B> {
    fn clone(&self) -> Config<C, B> {
        Config {
            connector: self.connector.clone(),
            .. *self
        }
    }
}

#[cfg(test)]
mod tests {
    /*
    use std::io::Read;
    use header::Server;
    use super::{Client};
    use super::pool::Pool;
    use Url;

    mock_connector!(Issue640Connector {
        b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\n",
        b"GET",
        b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\n",
        b"HEAD",
        b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\n",
        b"POST"
    });

    // see issue #640
    #[test]
    fn test_head_response_body_keep_alive() {
        let client = Client::with_connector(Pool::with_connector(Default::default(), Issue640Connector));

        let mut s = String::new();
        client.get("http://127.0.0.1").send().unwrap().read_to_string(&mut s).unwrap();
        assert_eq!(s, "GET");

        let mut s = String::new();
        client.head("http://127.0.0.1").send().unwrap().read_to_string(&mut s).unwrap();
        assert_eq!(s, "");

        let mut s = String::new();
        client.post("http://127.0.0.1").send().unwrap().read_to_string(&mut s).unwrap();
        assert_eq!(s, "POST");
    }
    */
}
