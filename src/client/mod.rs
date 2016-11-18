//! HTTP Client
//!
//! The HTTP `Client` uses asynchronous IO, and utilizes the `Handler` trait
//! to convey when IO events are available for a given request.

use std::cell::RefCell;
use std::collections::{VecDeque, HashMap};
use std::fmt;
use std::io;
use std::marker::PhantomData;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use futures::{Async, Poll, Future};
use tokio::net::TcpStream;
use tokio::reactor::Handle;
use tokio_proto::{pipeline, Message, Body};
pub use tokio_service::Service;

use header::Host;
use http::{self, Conn, RequestHead, ClientTransaction};
use method::Method;
use net::Transport;
use uri::RequestUri;
use {Url};

pub use self::connect::{DefaultConnector, HttpConnector};
pub use self::request::Request;
pub use self::response::Response;

mod connect;
mod dns;
mod request;
mod response;

/// A Client to make outgoing HTTP requests.
pub struct Client<C> {
    connector: C,
    handle: Handle,
    sockets: RefCell<Vec<::tokio_proto::Client<http::MessageHead<http::RequestLine>, http::MessageHead<http::RawStatus>, ::futures::stream::Receiver<http::Chunk, ::Error>, Body<http::Chunk, ::Error>, ::Error>>>,
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
            connector: HttpConnector::new(handle, 4),
            handle: handle.clone(),
            sockets: RefCell::new(Vec::new()),
        })
    }
}

impl Client<DefaultConnector> {
    /// Create a new client with a specific connector.
    fn configured<C>(config: Config<C>) -> ::Result<Client<C>> {
        unimplemented!("Client::configured")
    }

    pub fn get(&self, url: Url) -> FutureResponse {
        self.request(Request::new(Method::Get, url))
    }

    /// Build a new request using this Client.
    pub fn request(&self, req: Request) -> FutureResponse {
        self.call(req)
    }
}

pub struct FutureResponse(Box<Future<Item=Response, Error=::Error> + Send + 'static>);

impl Future for FutureResponse {
    type Item = Response;
    type Error = ::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        self.0.poll()
    }
}

impl Service for Client<DefaultConnector> {
    type Request = Request;
    type Response = Response;
    type Error = ::Error;
    type Future = FutureResponse;

    fn call(&self, mut req: Request) -> Self::Future {
        let url = match req.uri() {
            &::RequestUri::AbsoluteUri(ref u) => u.clone(),
            _ => unimplemented!("RequestUri::*")
        };
        req.headers_mut().set(::header::Host {
            hostname: url.host_str().unwrap().to_owned(),
            port: None,
        });
        req.headers_mut().set(::header::ContentLength(0));
        let client = pipeline::connect::<_, _, ::futures::stream::Receiver<::http::Chunk, ::Error>>(self.connector.call(url).map(Conn::<TcpStream, ClientTransaction>::new), &self.handle);
        let req = client.call(Message::WithoutBody(req.head));
        self.sockets.borrow_mut().push(client);
        FutureResponse(req.map(|msg| {
            match msg {
                Message::WithoutBody(head) => response::new(head, None),
                Message::WithBody(head, body) => response::new(head, Some(body)),
            }
        }).boxed())
        //self.connector.call(req.url()).and_then(do_stuff)
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

impl<C> Config<C> {
    /// Set the `Connect` type to be used.
    #[inline]
    pub fn connector<CC>(self, val: CC) -> Config<CC> {
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

/*
/// An error that can occur when trying to queue a request.
#[derive(Debug)]
pub struct ClientError<H>(Option<(Url, H)>);

impl<H> ClientError<H> {
    /// If the event loop was down, the `Url` and `Handler` can be recovered
    /// from this method.
    pub fn recover(self) -> Option<(Url, H)> {
        self.0
    }
}

impl<H: fmt::Debug + ::std::any::Any> ::std::error::Error for ClientError<H> {
    fn description(&self) -> &str {
        "Cannot queue request"
    }
}

impl<H> fmt::Display for ClientError<H> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("Cannot queue request")
    }
}

enum Notify<T> {
    Connect(Url, T),
    Shutdown,
}
*/

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
