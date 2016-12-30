//! HTTP Client
//!
//! The HTTP `Client` uses asynchronous IO, and utilizes the `Handler` trait
//! to convey when IO events are available for a given request.

use std::cell::RefCell;
use std::fmt;
use std::io;
use std::rc::Rc;
use std::time::Duration;

use futures::{Poll, Async, Future};
use futures::sync::oneshot;
use tokio::io::Io;
use tokio::reactor::Handle;
use tokio_proto::BindClient;
use tokio_proto::streaming::Message;
use tokio_proto::streaming::pipeline::ClientProto;
use tokio_proto::util::client_proxy::ClientProxy;
pub use tokio_service::Service;

use header::{Headers, Host};
use http::{self, TokioBody};
use method::Method;
use self::pool::{Pool, Pooled};
use uri::RequestUri;
use {Url};

pub use self::connect::{DefaultConnector, HttpConnector, Connect};
pub use self::request::Request;
pub use self::response::Response;

mod connect;
mod dns;
mod pool;
mod request;
mod response;

/// A Client to make outgoing HTTP requests.
// If the Connector is clone, then the Client can be clone easily.
#[derive(Clone)]
pub struct Client<C> {
    connector: C,
    handle: Handle,
    pool: Pool<TokioClient>,
}

impl Client<DefaultConnector> {
    /// Configure a Client.
    ///
    /// # Example
    ///
    /// ```dont_run
    /// # use hyper::Client;
    /// let client = Client::configure()
    ///     .keep_alive(true)
    ///     .max_sockets(10_000)
    ///     .build().unwrap();
    /// ```
    #[inline]
    pub fn configure() -> Config<DefaultConnector> {
        Config::default()
    }
}

impl Client<DefaultConnector> {
    /// Create a new Client with the default config.
    #[inline]
    pub fn new(handle: &Handle) -> ::Result<Client<DefaultConnector>> {
        //Client::configure().build()
        Ok(Client {
            connector: DefaultConnector::new(handle, 4),
            handle: handle.clone(),
            pool: Pool::new(Duration::from_secs(90)),
        })
    }
}

impl<C: Connect> Client<C> {
    /// Create a new client with a specific connector.
    fn configured(_config: Config<C>) -> ::Result<Client<C>> {
        unimplemented!("Client::configured")
    }

    /// Send a GET Request using this Client.
    pub fn get(&mut self, url: Url) -> FutureResponse {
        self.request(Request::new(Method::Get, url))
    }

    /// Send a constructed Request using this Client.
    pub fn request(&mut self, req: Request) -> FutureResponse {
        self.call(req)
    }
}

/// A `Future` that will resolve to an HTTP Response.
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

impl<C: Connect> Service for Client<C> {
    type Request = Request;
    type Response = Response;
    type Error = ::Error;
    type Future = FutureResponse;

    fn call(&mut self, req: Request) -> Self::Future {
        let url = match req.uri() {
            &::RequestUri::AbsoluteUri(ref u) => u.clone(),
            _ => unimplemented!("RequestUri::*")
        };

        let (mut head, body) = request::split(req);
        let mut headers = Headers::new();
        headers.set(Host {
            hostname: url.host_str().unwrap().to_owned(),
            port: url.port().or(None),
        });
        headers.extend(head.headers.iter());
        head.subject.1 = RequestUri::AbsolutePath {
            path: url.path().to_owned(),
            query: url.query().map(ToOwned::to_owned),
        };
        head.headers = headers;

        let checkout = self.pool.checkout(&url[..::url::Position::BeforePath]);
        let connect = {
            let handle = self.handle.clone();
            let pool = self.pool.clone();
            let pool_key = Rc::new(url[..::url::Position::BeforePath].to_owned());
            self.connector.connect(url)
                .map(move |io| {
                    let (tx, rx) = oneshot::channel();
                    let client = HttpClient {
                        client_rx: RefCell::new(Some(rx)),
                    }.bind_client(&handle, io);
                    let pooled = pool.pooled(pool_key, client);
                    tx.complete(pooled.clone());
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
        let req = race.and_then(move |mut client| {
            let msg = match body {
                Some(body) => {
                    Message::WithBody(head, body.into())
                },
                None => Message::WithoutBody(head),
            };
            client.call(msg)
        });
        FutureResponse(Box::new(req.map(|msg| {
            match msg {
                Message::WithoutBody(head) => response::new(head, None),
                Message::WithBody(head, body) => response::new(head, Some(body.into())),
            }
        })))
    }

}

impl<C> fmt::Debug for Client<C> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad("Client")
    }
}

type TokioClient = ClientProxy<Message<http::RequestHead, TokioBody>, Message<http::ResponseHead, TokioBody>, ::Error>;

struct HttpClient {
    client_rx: RefCell<Option<oneshot::Receiver<Pooled<TokioClient>>>>,
}

impl<T: Io + 'static> ClientProto<T> for HttpClient {
    type Request = http::RequestHead;
    type RequestBody = http::Chunk;
    type Response = http::ResponseHead;
    type ResponseBody = http::Chunk;
    type Error = ::Error;
    type Transport = http::Conn<T, http::ClientTransaction, Pooled<TokioClient>>;
    type BindTransport = BindingClient<T>;

    fn bind_transport(&self, io: T) -> Self::BindTransport {
        BindingClient {
            rx: self.client_rx.borrow_mut().take().expect("client_rx was lost"),
            io: Some(io),
        }
    }
}

struct BindingClient<T> {
    rx: oneshot::Receiver<Pooled<TokioClient>>,
    io: Option<T>,
}

impl<T: Io + 'static> Future for BindingClient<T> {
    type Item = http::Conn<T, http::ClientTransaction, Pooled<TokioClient>>;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        match self.rx.poll() {
            Ok(Async::Ready(client)) => Ok(Async::Ready(
                    http::Conn::new(self.io.take().expect("binding client io lost"), client)
            )),
            Ok(Async::NotReady) => Ok(Async::NotReady),
            Err(_canceled) => unreachable!(),
        }
    }
}

/// Configuration for a Client
#[derive(Debug, Clone)]
pub struct Config<C> {
    connect_timeout: Duration,
    connector: C,
    keep_alive: bool,
    keep_alive_timeout: Option<Duration>,
    //TODO: make use of max_idle config
    max_idle: usize,
    max_sockets: usize,
    dns_workers: usize,
}

impl<C: Connect> Config<C> {
    /// Set the `Connect` type to be used.
    #[inline]
    pub fn connector<CC: Connect>(self, val: CC) -> Config<CC> {
        Config {
            connect_timeout: self.connect_timeout,
            connector: val,
            keep_alive: self.keep_alive,
            keep_alive_timeout: Some(Duration::from_secs(60 * 2)),
            max_idle: self.max_idle,
            max_sockets: self.max_sockets,
            dns_workers: self.dns_workers,
        }
    }

    /// Enable or disable keep-alive mechanics.
    ///
    /// Default is enabled.
    #[inline]
    pub fn keep_alive(mut self, val: bool) -> Config<C> {
        self.keep_alive = val;
        self
    }

    /// Set an optional timeout for idle sockets being kept-alive.
    ///
    /// Pass `None` to disable timeout.
    ///
    /// Default is 2 minutes.
    #[inline]
    pub fn keep_alive_timeout(mut self, val: Option<Duration>) -> Config<C> {
        self.keep_alive_timeout = val;
        self
    }

    /// Set the max table size allocated for holding on to live sockets.
    ///
    /// Default is 1024.
    #[inline]
    pub fn max_sockets(mut self, val: usize) -> Config<C> {
        self.max_sockets = val;
        self
    }

    /// Set the timeout for connecting to a URL.
    ///
    /// Default is 10 seconds.
    #[inline]
    pub fn connect_timeout(mut self, val: Duration) -> Config<C> {
        self.connect_timeout = val;
        self
    }

    /// Set number of Dns workers to use for this client
    ///
    /// Default is 4
    #[inline]
    pub fn dns_workers(mut self, workers: usize) -> Config<C> {
        self.dns_workers = workers;
        self
    }

    /// Construct the Client with this configuration.
    #[inline]
    pub fn build(self) -> ::Result<Client<C>> {
        Client::configured(self)
    }
}

impl Default for Config<DefaultConnector> {
    fn default() -> Config<DefaultConnector> {
        unimplemented!("Config::default")
        /*
        Config {
            connect_timeout: Duration::from_secs(10),
            connector: DefaultConnector::default(),
            keep_alive: true,
            keep_alive_timeout: Some(Duration::from_secs(60 * 2)),
            max_idle: 5,
            max_sockets: 1024,
            dns_workers: 4,
        }
        */
    }
}

#[cfg(test)]
mod tests {
    /*
    use std::io::Read;
    use header::Server;
    use super::{Client};
    use super::pool::Pool;
    use url::Url;

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
