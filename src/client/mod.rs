//! HTTP Client
//!
//! The HTTP `Client` uses asynchronous IO, and utilizes the `Handler` trait
//! to convey when IO events are available for a given request.

use std::collections::{VecDeque, HashMap};
use std::fmt;
use std::io;
use std::marker::PhantomData;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use rotor::{self, Scope, EventSet, PollOpt};

use header::Host;
use http::{self, Next, RequestHead, ReadyResult};
use net::Transport;
use uri::RequestUri;
use {Url};

pub use self::connect::{Connect, DefaultConnector, HttpConnector, HttpsConnector, DefaultTransport};
pub use self::request::Request;
pub use self::response::Response;

mod connect;
mod dns;
mod request;
mod response;

/// A Client to make outgoing HTTP requests.
pub struct Client<H> {
    tx: http::channel::Sender<Notify<H>>,
}

impl<H> Clone for Client<H> {
    fn clone(&self) -> Client<H> {
        Client {
            tx: self.tx.clone()
        }
    }
}

impl<H> fmt::Debug for Client<H> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad("Client")
    }
}

impl<H> Client<H> {
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

impl<H: Handler<<DefaultConnector as Connect>::Output>> Client<H> {
    /// Create a new Client with the default config.
    #[inline]
    pub fn new() -> ::Result<Client<H>> {
        Client::<H>::configure().build()
    }
}

impl<H: Send> Client<H> {
    /// Create a new client with a specific connector.
    fn configured<T, C>(config: Config<C>) -> ::Result<Client<H>>
    where H: Handler<T>,
          T: Transport,
          C: Connect<Output=T> + Send + 'static {
        let mut rotor_config = rotor::Config::new();
        rotor_config.slab_capacity(config.max_sockets);
        rotor_config.mio().notify_capacity(config.max_sockets);
        let keep_alive = config.keep_alive;
        let connect_timeout = config.connect_timeout;
        let mut loop_ = try!(rotor::Loop::new(&rotor_config));
        let mut notifier = None;
        let mut connector = config.connector;
        connector.dns_workers(config.dns_workers);
        {
            let not = &mut notifier;
            loop_.add_machine_with(move |scope| {
                let (tx, rx) = http::channel::new(scope.notifier());
                let (dns_tx, dns_rx) = http::channel::share(&tx);
                *not = Some(tx);
                connector.register(Registration {
                    notify: (dns_tx, dns_rx),
                });
                rotor::Response::ok(ClientFsm::Connector(connector, rx))
            }).unwrap();
        }

        let notifier = notifier.expect("loop.add_machine_with failed");
        let _handle = try!(thread::Builder::new().name("hyper-client".to_owned()).spawn(move || {
            loop_.run(Context {
                connect_timeout: connect_timeout,
                keep_alive: keep_alive,
                idle_conns: HashMap::new(),
                queue: HashMap::new(),
                awaiting_slot: VecDeque::new(),
            }).unwrap()
        }));

        Ok(Client {
            //handle: Some(handle),
            tx: notifier,
        })
    }

    /// Build a new request using this Client.
    ///
    /// ## Error
    ///
    /// If the event loop thread has died, or the queue is full, a `ClientError`
    /// will be returned.
    pub fn request(&self, url: Url, handler: H) -> Result<(), ClientError<H>> {
        self.tx.send(Notify::Connect(url, handler)).map_err(|e| {
            match e.0 {
                Some(Notify::Connect(url, handler)) => ClientError(Some((url, handler))),
                _ => ClientError(None)
            }
        })
    }

    /// Close the Client loop.
    pub fn close(self) {
        // Most errors mean that the Receivers are already dead, which would
        // imply the EventLoop panicked.
        let _ = self.tx.send(Notify::Shutdown);
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

impl<C> Config<C> where C: Connect + Send + 'static {
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
    pub fn build<H: Handler<C::Output>>(self) -> ::Result<Client<H>> {
        Client::configured(self)
    }
}

impl Default for Config<DefaultConnector> {
    fn default() -> Config<DefaultConnector> {
        Config {
            connect_timeout: Duration::from_secs(10),
            connector: DefaultConnector::default(),
            keep_alive: true,
            keep_alive_timeout: Some(Duration::from_secs(60 * 2)),
            max_idle: 5,
            max_sockets: 1024,
            dns_workers: 4,
        }
    }
}

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

/// A trait to react to client events that happen for each message.
///
/// Each event handler returns it's desired `Next` action.
pub trait Handler<T: Transport>: Send + 'static {
    /// This event occurs first, triggering when a `Request` head can be written..
    fn on_request(&mut self, request: &mut Request) -> http::Next;
    /// This event occurs each time the `Request` is ready to be written to.
    fn on_request_writable(&mut self, request: &mut http::Encoder<T>) -> http::Next;
    /// This event occurs after the first time this handler signals `Next::read()`,
    /// and a Response has been parsed.
    fn on_response(&mut self, response: Response) -> http::Next;
    /// This event occurs each time the `Response` is ready to be read from.
    fn on_response_readable(&mut self, response: &mut http::Decoder<T>) -> http::Next;

    /// This event occurs whenever an `Error` occurs outside of the other events.
    ///
    /// This could IO errors while waiting for events, or a timeout, etc.
    fn on_error(&mut self, err: ::Error) -> http::Next {
        debug!("default Handler.on_error({:?})", err);
        http::Next::remove()
    }

    /// This event occurs when this Handler has requested to remove the Transport.
    fn on_remove(self, _transport: T) where Self: Sized {
        debug!("default Handler.on_remove");
    }

    /// Receive a `Control` to manage waiting for this request.
    fn on_control(&mut self, _: http::Control) {
        debug!("default Handler.on_control()");
    }
}

struct Message<H: Handler<T>, T: Transport> {
    handler: H,
    url: Option<Url>,
    _marker: PhantomData<T>,
}

impl<H: Handler<T>, T: Transport> http::MessageHandler<T> for Message<H, T> {
    type Message = http::ClientMessage;

    fn on_outgoing(&mut self, head: &mut RequestHead) -> Next {
        let url = self.url.take().expect("Message.url is missing");
        if let Some(host) = url.host_str() {
            head.headers.set(Host {
                hostname: host.to_owned(),
                port: url.port(),
            });
        }
        head.subject.1 = RequestUri::AbsolutePath {
            path: url.path().to_owned(),
            query: url.query().map(|q| q.to_owned()),
        };
        let mut req = self::request::new(head);
        self.handler.on_request(&mut req)
    }

    fn on_encode(&mut self, transport: &mut http::Encoder<T>) -> Next {
        self.handler.on_request_writable(transport)
    }

    fn on_incoming(&mut self, head: http::ResponseHead, _: &T) -> Next {
        trace!("on_incoming {:?}", head);
        let resp = response::new(head);
        self.handler.on_response(resp)
    }

    fn on_decode(&mut self, transport: &mut http::Decoder<T>) -> Next {
        self.handler.on_response_readable(transport)
    }

    fn on_error(&mut self, error: ::Error) -> Next {
        self.handler.on_error(error)
    }

    fn on_remove(self, transport: T) {
        self.handler.on_remove(transport);
    }
}

struct Context<K, H, C: Connect> {
    connect_timeout: Duration,
    keep_alive: bool,
    idle_conns: HashMap<K, VecDeque<http::Control>>,
    queue: HashMap<K, VecDeque<Queued<H>>>,
    awaiting_slot: VecDeque<(C::Key, C::Output)>,
}

/// Macro for advancing state of a ClientFsm::Socket
///
/// This was previously a method on Context, but due to eviction needs, this
/// block now needs access to the registration APIs on rotor::Scope.
macro_rules! conn_response {
    ($scope:expr, $conn:expr, $time:expr) => {{
        match $conn {
            Some((conn, timeout)) => {
                //TODO: HTTP2: a connection doesn't need to be idle to be used for a second stream
                if conn.is_idle() {
                    $scope.idle_conns.entry(conn.key().clone()).or_insert_with(VecDeque::new)
                        .push_back(conn.control());
                }
                match timeout {
                    Some(dur) => rotor::Response::ok(ClientFsm::Socket(conn))
                        .deadline($time + dur),
                    None => rotor::Response::ok(ClientFsm::Socket(conn)),
                }

            }
            None => {
                if let Some((key, socket)) = $scope.awaiting_slot.pop_front() {
                    rotor_try!($scope.register(&socket, EventSet::writable(), PollOpt::level()));
                    rotor::Response::ok(ClientFsm::Connecting((key, socket)))
                } else {
                    rotor::Response::done()
                }
            }
        }
    }}
}

impl<K: http::Key, H, C: Connect> Context<K, H, C> {
    fn pop_queue(&mut self, key: &K) -> Option<Queued<H>> {
        let mut should_remove = false;
        let queued = {
            self.queue.get_mut(key).and_then(|vec| {
                let queued = vec.pop_front();
                if vec.is_empty() {
                    should_remove = true;
                }
                queued
            })
        };
        if should_remove {
            self.queue.remove(key);
        }

        queued
    }
}

impl<K, H, T, C> http::MessageHandlerFactory<K, T> for Context<K, H, C>
    where K: http::Key,
          H: Handler<T>,
          T: Transport,
          C: Connect
{
    type Output = Message<H, T>;

    fn create(&mut self, seed: http::Seed<K>) -> Option<Self::Output> {
        let key = seed.key();
        self.pop_queue(key).map(|queued| {
            let (url, mut handler) = (queued.url, queued.handler);
            handler.on_control(seed.control());

            Message {
                handler: handler,
                url: Some(url),
                _marker: PhantomData,
            }
        })
    }

    fn keep_alive_interest(&self) -> Next {
        Next::wait()
    }
}

enum Notify<T> {
    Connect(Url, T),
    Shutdown,
}

enum ClientFsm<C, H>
where C: Connect,
      C::Output: Transport,
      H: Handler<C::Output> {
    Connector(C, http::channel::Receiver<Notify<H>>),
    Connecting((C::Key, C::Output)),
    Socket(http::Conn<C::Key, C::Output, Message<H, C::Output>>)
}

unsafe impl<C, H> Send for ClientFsm<C, H>
where
    C: Connect + Send,
    //C::Key, // Key doesn't need to be Send
    C::Output: Transport, // Tranport doesn't need to be Send
    H: Handler<C::Output> + Send
{}

impl<C, H> rotor::Machine for ClientFsm<C, H>
where C: Connect,
      C::Key: fmt::Debug,
      C::Output: Transport,
      H: Handler<C::Output> {
    type Context = Context<C::Key, H, C>;
    type Seed = (C::Key, C::Output);

    fn create(seed: Self::Seed, scope: &mut Scope<Self::Context>) -> rotor::Response<Self, rotor::Void> {
        rotor_try!(scope.register(&seed.1, EventSet::writable(), PollOpt::level()));
        rotor::Response::ok(ClientFsm::Connecting(seed))
    }

    fn ready(self, events: EventSet, scope: &mut Scope<Self::Context>) -> rotor::Response<Self, Self::Seed> {
        match self {
            ClientFsm::Socket(conn) => {
                let mut conn = Some(conn);
                loop {
                    match conn.take().unwrap().ready(events, scope) {
                        ReadyResult::Done(res) => {
                            let now = scope.now();
                            return conn_response!(scope, res, now);
                        },
                        ReadyResult::Continue(c) => conn = Some(c),
                    }
                }
            },
            ClientFsm::Connecting(mut seed) => {
                if events.is_error() || events.is_hup() {
                    if let Some(err) = seed.1.take_socket_error().err() {
                        debug!("error while connecting: {:?}", err);
                        scope.pop_queue(&seed.0).map(move |mut queued| queued.handler.on_error(::Error::Io(err)));
                        rotor::Response::done()
                    } else {
                        trace!("connecting is_error, but no socket error");
                        rotor::Response::ok(ClientFsm::Connecting(seed))
                    }
                } else if events.is_writable() {
                    if scope.queue.contains_key(&seed.0) {
                        trace!("connected and writable {:?}", seed.0);
                        rotor::Response::ok(
                            ClientFsm::Socket(
                                http::Conn::new(seed.0, seed.1, Next::write().timeout(scope.connect_timeout), scope.notifier())
                                    .keep_alive(scope.keep_alive)
                            )
                        )
                    } else {
                        trace!("connected, but queued handler is gone: {:?}", seed.0); // probably took too long connecting
                        rotor::Response::done()
                    }
                } else {
                    // spurious?
                    rotor::Response::ok(ClientFsm::Connecting(seed))
                }
            }
            ClientFsm::Connector(..) => {
                unreachable!("Connector can never be ready")
            },
        }
    }

    fn spawned(self, scope: &mut Scope<Self::Context>) -> rotor::Response<Self, Self::Seed> {
        match self {
            ClientFsm::Connector(..) => self.connect(scope),
            other => rotor::Response::ok(other)
        }
    }

    fn spawn_error(
        self,
        scope: &mut Scope<Self::Context>,
        error: rotor::SpawnError<Self::Seed>
    ) -> rotor::Response<Self, Self::Seed> {
        // see if there's an idle connections that can be terminated. If yes, put this seed on a
        // list waiting for empty slot.
        if let rotor::SpawnError::NoSlabSpace((key, socket)) = error {
            if let Some(mut queued) = scope.pop_queue(&key) {
                trace!("attempting to remove an idle socket");
                // Remove an idle connection. Any connection. Just make some space
                // for the new request.
                let mut remove_keys = Vec::new();
                let mut found_idle = false;

                // Check all idle connections regardless of origin
                for (key, idle) in scope.idle_conns.iter_mut() {
                    while let Some(ctrl) = idle.pop_front() {
                        // Signal connection to close. An err here means the
                        // socket is already dead can should be tossed.
                        if ctrl.ready(Next::remove()).is_ok() {
                            found_idle = true;
                            break;
                        }
                    }

                    // This list is empty, mark it for removal
                    if idle.is_empty() {
                        remove_keys.push(key.to_owned());
                    }

                    // if found, stop looking for an idle connection.
                    if found_idle {
                        break;
                    }
                }

                trace!("idle conns: {:?}", scope.idle_conns);

                // Remove empty idle lists.
                for key in &remove_keys {
                    scope.idle_conns.remove(&key);
                }

                if found_idle {
                    // A socket should be evicted soon; put it on a queue to
                    // consume newly freed slot. Also need to put the Queued<H>
                    // back onto front of queue.
                    scope.awaiting_slot.push_back((key.clone(), socket));
                    scope.queue
                        .entry(key)
                        .or_insert_with(VecDeque::new)
                        .push_back(queued);
                } else {
                    // Couldn't evict a socket, just run the error handler.
                    debug!("Error spawning state machine; slab full and no sockets idle");
                    let _ = queued.handler.on_error(::Error::Full);
                }
            }
        }

        self.connect(scope)
    }

    fn timeout(self, scope: &mut Scope<Self::Context>) -> rotor::Response<Self, Self::Seed> {
        trace!("timeout now = {:?}", scope.now());
        match self {
            ClientFsm::Connector(..) => {
                let now = scope.now();
                let mut empty_keys = Vec::new();
                {
                    for (key, mut vec) in &mut scope.queue {
                        while !vec.is_empty() && vec[0].deadline <= now {
                            vec.pop_front()
                               .map(|mut queued| queued.handler.on_error(::Error::Timeout));
                        }
                        if vec.is_empty() {
                            empty_keys.push(key.clone());
                        }
                    }
                }
                for key in &empty_keys {
                    scope.queue.remove(key);
                }
                match self.deadline(scope) {
                    Some(deadline) => {
                        rotor::Response::ok(self).deadline(deadline)
                    },
                    None => rotor::Response::ok(self)
                }
            }
            ClientFsm::Connecting(..) => unreachable!(),
            ClientFsm::Socket(conn) => {
                let res = conn.timeout(scope);
                let now = scope.now();
                conn_response!(scope, res, now)
            }
        }
    }

    fn wakeup(self, scope: &mut Scope<Self::Context>) -> rotor::Response<Self, Self::Seed> {
        match self {
            ClientFsm::Connector(..) => {
                self.connect(scope)
            },
            ClientFsm::Socket(conn) => {
                let res = conn.wakeup(scope);
                let now = scope.now();
                conn_response!(scope, res, now)
            },
            ClientFsm::Connecting(..) => unreachable!("connecting sockets should not be woken up")
        }
    }
}

impl<C, H> ClientFsm<C, H>
where C: Connect,
      C::Key: fmt::Debug,
      C::Output: Transport,
      H: Handler<C::Output> {
    fn connect(self, scope: &mut rotor::Scope<<Self as rotor::Machine>::Context>) -> rotor::Response<Self, <Self as rotor::Machine>::Seed> {
        match self {
            ClientFsm::Connector(mut connector, rx) => {
                if let Some((key, res)) = connector.connected() {
                    match res {
                        Ok(socket) => {
                            trace!("connecting {:?}", key);
                            return rotor::Response::spawn(ClientFsm::Connector(connector, rx), (key, socket));
                        },
                        Err(e) => {
                            trace!("connect error = {:?}", e);
                            scope.pop_queue(&key).map(|mut queued| queued.handler.on_error(::Error::Io(e)));
                        }
                    }
                }
                loop {
                    match rx.try_recv() {
                        Ok(Notify::Connect(url, mut handler)) => {
                            // check pool for sockets to this domain
                            if let Some(key) = connector.key(&url) {
                                let mut remove_idle = false;
                                let mut woke_up = false;
                                if let Some(mut idle) = scope.idle_conns.get_mut(&key) {
                                    while let Some(ctrl) = idle.pop_front() {
                                        // err means the socket has since died
                                        if ctrl.ready(Next::write()).is_ok() {
                                            woke_up = true;
                                            break;
                                        }
                                    }
                                    remove_idle = idle.is_empty();
                                }
                                if remove_idle {
                                    scope.idle_conns.remove(&key);
                                }

                                if woke_up {
                                    trace!("woke up idle conn for '{}'", url);
                                    let deadline = scope.now() + scope.connect_timeout;
                                    scope.queue
                                        .entry(key)
                                        .or_insert_with(VecDeque::new)
                                        .push_back(Queued {
                                            deadline: deadline,
                                            handler: handler,
                                            url: url
                                        });
                                    continue;
                                }
                            } else {
                                // this connector cannot handle this url anyways
                                let _ = handler.on_error(io::Error::new(io::ErrorKind::InvalidInput, "invalid url for connector").into());
                                continue;
                            }
                            // no exist connection, call connector
                            match connector.connect(&url) {
                                Ok(key) => {
                                    let deadline = scope.now() + scope.connect_timeout;
                                    scope.queue
                                        .entry(key)
                                        .or_insert_with(VecDeque::new)
                                        .push_back(Queued {
                                            deadline: deadline,
                                            handler: handler,
                                            url: url
                                        });
                                }
                                Err(e) => {
                                    let _todo = handler.on_error(e.into());
                                    trace!("Connect error, next={:?}", _todo);
                                    continue;
                                }
                            }
                        }
                        Ok(Notify::Shutdown) => {
                            scope.shutdown_loop();
                            return rotor::Response::done()
                        },
                        Err(mpsc::TryRecvError::Disconnected) => {
                            // if there is no way to send additional requests,
                            // what more can the loop do? i suppose we should
                            // shutdown.
                            scope.shutdown_loop();
                            return rotor::Response::done()
                        }
                        Err(mpsc::TryRecvError::Empty) => {
                            // spurious wakeup or loop is done
                            let fsm = ClientFsm::Connector(connector, rx);
                            return match fsm.deadline(scope) {
                                Some(deadline) => {
                                    rotor::Response::ok(fsm).deadline(deadline)
                                },
                                None => rotor::Response::ok(fsm)
                            };
                        }
                    }
                }
            },
            other => rotor::Response::ok(other)
        }
    }

    fn deadline(&self, scope: &mut rotor::Scope<<Self as rotor::Machine>::Context>) -> Option<rotor::Time> {
        match *self {
            ClientFsm::Connector(..) => {
                let mut earliest = None;
                for vec in scope.queue.values() {
                    for queued in vec {
                        match earliest {
                            Some(ref mut earliest) => {
                                if queued.deadline < *earliest {
                                    *earliest = queued.deadline;
                                }
                            }
                            None => earliest = Some(queued.deadline)
                        }
                    }
                }
                trace!("deadline = {:?}, now = {:?}", earliest, scope.now());
                earliest
            }
            _ => None
        }
    }
}

struct Queued<H> {
    deadline: rotor::Time,
    handler: H,
    url: Url,
}

#[doc(hidden)]
#[allow(missing_debug_implementations)]
pub struct Registration {
    notify: (http::channel::Sender<self::dns::Answer>, http::channel::Receiver<self::dns::Answer>),
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
