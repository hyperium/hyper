//! HTTP Client
use std::default::Default;
use std::fmt;
use std::io::{Read};
use std::marker::PhantomData;
use std::sync::mpsc;
use std::thread;

use rotor::{self, Scope, EventSet, PollOpt};

use url::ParseError as UrlError;

use header::Host;
use http::{self, Next, RequestHead};
use net::{Transport, Connect, DefaultConnector};
use uri::RequestUri;
use {Url};
use Error;

pub use self::request::Request;
pub use self::response::Response;

//mod pool;
mod request;
mod response;

/// A Client to use additional features with Requests.
///
/// Clients can handle things such as: redirect policy, connection pooling.
pub struct Client<H> {
    //handle: Option<thread::JoinHandle<()>>,
    notifier: (rotor::Notifier, mpsc::Sender<Notify<H>>),
}

impl<H> Clone for Client<H> {
    fn clone(&self) -> Client<H> {
        Client {
            notifier: self.notifier.clone()
        }
    }
}

impl<H> fmt::Debug for Client<H> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad("Client")
    }
}

impl<H: Handler<<DefaultConnector as Connect>::Output>> Client<H> {
    /// Create a new Client.
    pub fn new() -> ::Result<Client<H>> {
        Client::with_connector(DefaultConnector::default())
    }
}

impl<H> Client<H> {
    /// Create a new client with a specific connector.
    pub fn with_connector<T, C>(connector: C) -> ::Result<Client<H>>
    where H: Handler<T>,
          T: Transport + Send,
          C: Connect<Output=T> + Send + 'static {
          //C::Output: Transport + Send + 'static {
        let mut loop_ = try!(rotor::Loop::new(&rotor::Config::new()));
        let (tx, rx) = mpsc::channel();
        let mut notifier = None;
        {
            let not = &mut notifier;
            loop_.add_machine_with(move |scope| {
                *not = Some(scope.notifier());
                rotor::Response::ok(ClientFsm::Connector(connector, rx))
            }).unwrap();
        }

        let notifier = notifier.expect("loop.add_machine_with failed");
        let _handle = try!(thread::Builder::new().name("hyper-client".to_owned()).spawn(move || {
            loop_.run(Context {
                queue: Vec::new(),
                _marker: PhantomData,
            }).unwrap()
        }));

        Ok(Client {
            //handle: Some(handle),
            notifier: (notifier, tx),
        })
    }

    /// Build a new request using this Client.
    pub fn request(&self, url: Url, handler: H) {
        self.notifier.1.send(Notify::Connect(url, handler))
            .expect("event loop has panicked");
        self.notifier.0.wakeup()
            .expect("event loop notify queue is full, cannot recover");
    }

    /// Close the Client loop.
    pub fn close(self) {
        // Most errors mean that the Receivers are already dead, which would
        // imply the EventLoop panicked.
        let _ = self.notifier.1.send(Notify::Shutdown);
        let _ = self.notifier.0.wakeup();
    }
}

/*
impl Drop for Client {
    fn drop(&mut self) {
        self.handle.take().map(|handle| handle.join());
    }
}
*/

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

    /// Receive a `Control` to manage waiting for this request.
    fn on_control(&mut self, _: http::Control) {
        debug!("default Handler.on_control()");
    }
}

struct UrlParts {
    host: String,
    port: u16,
    path: RequestUri,
}

struct Message<H: Handler<T>, T: Transport> {
    handler: H,
    url: Option<UrlParts>,
    _marker: PhantomData<T>,
}

impl<H: Handler<T>, T: Transport> http::MessageHandler<T> for Message<H, T> {
    type Message = http::ClientMessage;

    fn on_outgoing(&mut self, head: &mut RequestHead) -> Next {
        let url = self.url.take().expect("Message.url is missing");
        head.headers.set(Host {
            hostname: url.host,
            port: Some(url.port),
        });
        head.subject.1 = url.path;
        let mut req = self::request::new(head);
        self.handler.on_request(&mut req)
    }

    fn on_encode(&mut self, transport: &mut http::Encoder<T>) -> Next {
        self.handler.on_request_writable(transport)
    }

    fn on_incoming(&mut self, head: http::ResponseHead) -> Next {
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
}

struct Context<H: Handler<T>,T: Transport> {
    queue: Vec<(UrlParts, H)>,
    _marker: PhantomData<T>,
}

impl<H: Handler<T>, T: Transport> http::MessageHandlerFactory<T> for Context<H, T> {
    type Output = Message<H, T>;

    fn create(&mut self, ctrl: http::Control) -> Self::Output {
        let (url, mut handler) = self.queue.remove(0);
        handler.on_control(ctrl);
        Message {
            handler: handler,
            url: Some(url),
            _marker: PhantomData,
        }
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
    Connector(C, mpsc::Receiver<Notify<H>>),
    Socket(http::Conn<C::Output, Message<H, C::Output>>)
}

impl<C, H> rotor::Machine for ClientFsm<C, H>
where C: Connect,
      C::Output: Transport,
      H: Handler<C::Output> {
    type Context = Context<H, C::Output>;
    type Seed = C::Output;

    fn create(seed: Self::Seed, scope: &mut Scope<Self::Context>) -> rotor::Response<Self, rotor::Void> {
        rotor_try!(scope.register(&seed, EventSet::writable(), PollOpt::level()));
        rotor::Response::ok(ClientFsm::Socket(http::Conn::new(seed)))
    }

    fn ready(self, events: EventSet, scope: &mut Scope<Self::Context>) -> rotor::Response<Self, Self::Seed> {
        match self {
            ClientFsm::Connector(..) => {
                unreachable!()
            },
            ClientFsm::Socket(conn) => {
                match conn.ready(events, scope) {
                    Some((conn, None)) => rotor::Response::ok(ClientFsm::Socket(conn)),
                    Some((conn, Some(dur))) => {
                        rotor::Response::ok(ClientFsm::Socket(conn))
                            .deadline(scope.now() + dur)
                    }
                    None => rotor::Response::done()
                }
            }
        }
    }

    fn spawned(self, _: &mut Scope<Self::Context>) -> rotor::Response<Self, Self::Seed> {
        rotor::Response::ok(self)
    }

    fn timeout(self, scope: &mut Scope<Self::Context>) -> rotor::Response<Self, Self::Seed> {
        match self {
            ClientFsm::Connector(..) => unimplemented!("ClientFsm::Connector ontimeout"),
            ClientFsm::Socket(conn) => {
                match conn.timeout(scope) {
                    Some((conn, None)) => rotor::Response::ok(ClientFsm::Socket(conn)),
                    Some((conn, Some(dur))) => {
                        rotor::Response::ok(ClientFsm::Socket(conn))
                            .deadline(scope.now() + dur)
                    }
                    None => rotor::Response::done()
                }
            }
        }
    }

    fn wakeup(self, scope: &mut Scope<Self::Context>) -> rotor::Response<Self, Self::Seed> {
        match self {
            ClientFsm::Connector(connector, rx) => {
                match rx.try_recv() {
                    Ok(Notify::Connect(url, mut handler)) => {
                        // TODO: check pool for sockets to this domain
                        let (host, port) = match get_host_and_port(&url) {
                            Ok(v) => v,
                            Err(e) => {
                                let _todo = handler.on_error(e.into());
                                return rotor::Response::ok(ClientFsm::Connector(connector, rx));
                            }
                        };
                        let socket = match connector.connect(&host, port, &url.scheme) {
                            Ok(v) => v,
                            Err(e) => {
                                let _todo = handler.on_error(e.into());
                                return rotor::Response::ok(ClientFsm::Connector(connector, rx));
                            }
                        };
                        scope.queue.push((UrlParts {
                            host: host,
                            port: port,
                            path: RequestUri::AbsolutePath(url.serialize_path().unwrap())
                        }, handler));
                        rotor::Response::spawn(ClientFsm::Connector(connector, rx), socket)
                    }
                    Ok(Notify::Shutdown) => {
                        scope.shutdown_loop();
                        rotor::Response::done()
                    },
                    Err(mpsc::TryRecvError::Disconnected) => {
                        unimplemented!("Connector notifier disconnected");
                    }
                    Err(mpsc::TryRecvError::Empty) => {
                        // spurious wakeup
                        rotor::Response::ok(ClientFsm::Connector(connector, rx))
                    }
                }
            },
            ClientFsm::Socket(conn) => match conn.wakeup(scope) {
                Some((conn, None)) => rotor::Response::ok(ClientFsm::Socket(conn)),
                Some((conn, Some(dur))) => {
                    rotor::Response::ok(ClientFsm::Socket(conn))
                        .deadline(scope.now() + dur)
                }
                None => rotor::Response::done()
            }
        }
    }
}

fn get_host_and_port(url: &Url) -> ::Result<(String, u16)> {
    let host = match url.serialize_host() {
        Some(host) => host,
        None => return Err(Error::Uri(UrlError::EmptyHost))
    };
    trace!("host={:?}", host);
    let port = match url.port_or_default() {
        Some(port) => port,
        None => return Err(Error::Uri(UrlError::InvalidPort))
    };
    trace!("port={:?}", port);
    Ok((host, port))
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
