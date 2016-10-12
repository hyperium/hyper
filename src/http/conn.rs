use std::borrow::Cow;
use std::fmt;
use std::hash::Hash;
use std::io;
use std::marker::PhantomData;
use std::mem;
use std::time::Duration;

use rotor::{self, EventSet, PollOpt, Scope, Time};

use http::{self, h1, Http1Message, Encoder, Decoder, Next, Next_, Reg, Control};
use http::channel;
use http::internal::WriteBuf;
use http::buffer::Buffer;
use net::{Transport, Blocked};
use version::HttpVersion;

const MAX_BUFFER_SIZE: usize = 8192 + 4096 * 100;

/// This handles a connection, which will have been established over a
/// Transport (like a socket), and will likely include multiple
/// `Message`s over HTTP.
///
/// The connection will determine when a message begins and ends, creating
/// a new message `MessageHandler` for each one, as well as determine if this
/// connection can be kept alive after the message, or if it is complete.
pub struct Conn<K: Key, T: Transport, H: MessageHandler<T>>(Box<ConnInner<K, T, H>>);


/// `ConnInner` contains all of a connections state which Conn proxies for in a way
/// that allows Conn to maintain convenient move and self consuming method call
/// semantics but avoiding many costly memcpy calls.
struct ConnInner<K: Key, T: Transport, H: MessageHandler<T>> {
    buf: Buffer,
    ctrl: (channel::Sender<Next>, channel::Receiver<Next>),
    keep_alive_enabled: bool,
    key: K,
    state: State<H, T>,
    transport: T,
}

impl<K: Key, T: Transport, H: MessageHandler<T>> fmt::Debug for ConnInner<K, T, H> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Conn")
            .field("keep_alive_enabled", &self.keep_alive_enabled)
            .field("state", &self.state)
            .field("buf", &self.buf)
            .finish()
    }
}

impl<K: Key, T: Transport, H: MessageHandler<T>> fmt::Debug for Conn<K, T, H> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}


impl<K: Key, T: Transport, H: MessageHandler<T>> ConnInner<K, T, H> {
    /// Desired Register interest based on state of current connection.
    ///
    /// This includes the user interest, such as when they return `Next::read()`.
    fn interest(&self) -> Reg {
        match self.state {
            State::Closed => Reg::Remove,
            State::Init { interest, .. } => {
                interest.register()
            }
            State::Http1(Http1 { reading: Reading::Closed, writing: Writing::Closed, .. }) => {
                Reg::Remove
            }
            State::Http1(Http1 { ref reading, ref writing, .. }) => {
                let read = match *reading {
                    Reading::Parse |
                    Reading::Body(..) => Reg::Read,
                    Reading::Init |
                    Reading::Wait(..) |
                    Reading::KeepAlive |
                    Reading::Closed => Reg::Wait
                };

                let write = match *writing {
                    Writing::Head |
                    Writing::Chunk(..) |
                    Writing::Ready(..) => Reg::Write,
                    Writing::Init |
                    Writing::Wait(..) |
                    Writing::KeepAlive => Reg::Wait,
                    Writing::Closed => Reg::Wait,
                };

                match (read, write) {
                    (Reg::Read, Reg::Write) => Reg::ReadWrite,
                    (Reg::Read, Reg::Wait) => Reg::Read,
                    (Reg::Wait, Reg::Write) => Reg::Write,
                    (Reg::Wait, Reg::Wait) => Reg::Wait,
                    _ => unreachable!("bad read/write reg combo")
                }
            }
        }
    }

    /// Actual register action.
    ///
    /// Considers the user interest(), but also compares if the underlying
    /// transport is blocked(), and adjusts accordingly.
    fn register(&self) -> Reg {
        let reg = self.interest();
        match (reg, self.transport.blocked()) {
            (Reg::Remove, _) |
            (Reg::Wait, _) |
            (_, None) => reg,

            (_, Some(Blocked::Read)) => Reg::Read,
            (_, Some(Blocked::Write)) => Reg::Write,
        }
    }

    fn parse(&mut self) -> ::Result<http::MessageHead<<<H as MessageHandler<T>>::Message as Http1Message>::Incoming>> {
        match self.buf.read_from(&mut self.transport) {
            Ok(0) => {
                trace!("parse eof");
                return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "parse eof").into());
            }
            Ok(_) => {},
            Err(e) => match e.kind() {
                io::ErrorKind::WouldBlock => {},
                _ => return Err(e.into())
            }
        }
        match try!(http::parse::<<H as MessageHandler<T>>::Message, _>(self.buf.bytes())) {
            Some((head, len)) => {
                trace!("parsed {} bytes out of {}", len, self.buf.len());
                self.buf.consume(len);
                Ok(head)
            },
            None => {
                if self.buf.len() >= MAX_BUFFER_SIZE {
                    //TODO: Handler.on_too_large_error()
                    debug!("MAX_BUFFER_SIZE reached, closing");
                    Err(::Error::TooLarge)
                } else {
                    Err(io::Error::new(io::ErrorKind::WouldBlock, "incomplete parse").into())
                }
            },
        }
    }

    fn read<F: MessageHandlerFactory<K, T, Output=H>>(&mut self, scope: &mut Scope<F>, state: State<H, T>) -> State<H, T> {
         match state {
            State::Init { interest: Next_::Read, .. } => {
                let head = match self.parse() {
                    Ok(head) => head,
                    Err(::Error::Io(e)) => match e.kind() {
                        io::ErrorKind::WouldBlock |
                        io::ErrorKind::Interrupted => return state,
                        _ => {
                            debug!("io error trying to parse {:?}", e);
                            return State::Closed;
                        }
                    },
                    Err(e) => {
                        //TODO: send proper error codes depending on error
                        trace!("parse eror: {:?}", e);
                        return State::Closed;
                    }
                };
                let mut handler = match scope.create(Seed(&self.key, &self.ctrl.0)) {
                    Some(handler) => handler,
                    None => unreachable!()
                };
                match H::Message::decoder(&head) {
                    Ok(decoder) => {
                        trace!("decoder = {:?}", decoder);
                        let keep_alive = self.keep_alive_enabled && head.should_keep_alive();
                        let next = handler.on_incoming(head, &self.transport);
                        trace!("handler.on_incoming() -> {:?}", next);

                        let now = scope.now();
                        match next.interest {
                            Next_::Read => self.read(scope, State::Http1(Http1 {
                                handler: handler,
                                reading: Reading::Body(decoder),
                                writing: Writing::Init,
                                keep_alive: keep_alive,
                                timeout: next.timeout,
                                timeout_start: Some(now),
                                _marker: PhantomData,
                            })),
                            Next_::Write => State::Http1(Http1 {
                                handler: handler,
                                reading: if decoder.is_eof() {
                                    if keep_alive {
                                        Reading::KeepAlive
                                    } else {
                                        Reading::Closed
                                    }
                                } else {
                                    Reading::Wait(decoder)
                                },
                                writing: Writing::Head,
                                keep_alive: keep_alive,
                                timeout: next.timeout,
                                timeout_start: Some(now),
                                _marker: PhantomData,
                            }),
                            Next_::ReadWrite => self.read(scope, State::Http1(Http1 {
                                handler: handler,
                                reading: Reading::Body(decoder),
                                writing: Writing::Head,
                                keep_alive: keep_alive,
                                timeout: next.timeout,
                                timeout_start: Some(now),
                                _marker: PhantomData,
                            })),
                            Next_::Wait => State::Http1(Http1 {
                                handler: handler,
                                reading: Reading::Wait(decoder),
                                writing: Writing::Init,
                                keep_alive: keep_alive,
                                timeout: next.timeout,
                                timeout_start: Some(now),
                                _marker: PhantomData,
                            }),
                            Next_::End |
                            Next_::Remove => State::Closed,
                        }
                    },
                    Err(e) => {
                        debug!("error creating decoder: {:?}", e);
                        //TODO: update state from returned Next
                        //this would allow a Server to respond with a proper
                        //4xx code
                        let _ = handler.on_error(e);
                        State::Closed
                    }
                }
            },
            State::Init { interest: Next_::Wait, .. } => {
                match self.buf.read_from(&mut self.transport) {
                    Ok(0) => {
                        // End-of-file; connection was closed by peer
                        State::Closed
                    },
                    Ok(n) => {
                        // Didn't expect bytes here! Close the connection.
                        warn!("read {} bytes in State::Init with Wait interest", n);
                        State::Closed
                    },
                    Err(e) => match e.kind() {
                        io::ErrorKind::WouldBlock => {
                            // This is the expected case reading in this state
                            state
                        },
                        _ => {
                            warn!("socket error reading State::Init with Wait interest: {}", e);
                            State::Closed
                        }
                    }
                }
            },
            State::Init { .. } => {
                trace!("on_readable State::{:?}", state);
                state
            },
            State::Http1(mut http1) => {
                let next = match http1.reading {
                    Reading::Init => None,
                    Reading::Parse => match self.parse() {
                        Ok(head) => match H::Message::decoder(&head) {
                            Ok(decoder) => {
                                trace!("decoder = {:?}", decoder);
                                // if client request asked for keep alive,
                                // then it depends entirely on if the server agreed
                                if http1.keep_alive {
                                    http1.keep_alive = head.should_keep_alive();
                                }
                                let next = http1.handler.on_incoming(head, &self.transport);
                                http1.reading = Reading::Wait(decoder);
                                trace!("handler.on_incoming() -> {:?}", next);
                                Some(next)
                            },
                            Err(e) => {
                                debug!("error creating decoder: {:?}", e);
                                //TODO: respond with 400
                                return State::Closed;
                            }
                        },
                        Err(::Error::Io(e)) => match e.kind() {
                            io::ErrorKind::WouldBlock |
                            io::ErrorKind::Interrupted => None,
                            _ => {
                                debug!("io error trying to parse {:?}", e);
                                return State::Closed;
                            }
                        },
                        Err(e) => {
                            trace!("parse error: {:?}", e);
                            let _ = http1.handler.on_error(e);
                            return State::Closed;
                        }
                    },
                    Reading::Body(ref mut decoder) => {
                        let wrapped = if !self.buf.is_empty() {
                            super::Trans::Buf(self.buf.wrap(&mut self.transport))
                        } else {
                            super::Trans::Port(&mut self.transport)
                        };

                        Some(http1.handler.on_decode(&mut Decoder::h1(decoder, wrapped)))
                    },
                    _ => {
                        trace!("Conn.on_readable State::Http1(reading = {:?})", http1.reading);
                        None
                    }
                };
                let mut s = State::Http1(http1);
                if let Some(next) = next {
                    s.update(next, &**scope, Some(scope.now()));
                }
                trace!("Conn.on_readable State::Http1 completed, new state = State::{:?}", s);

                let again = match s {
                    State::Http1(Http1 { reading: Reading::Body(ref encoder), .. }) => encoder.is_eof(),
                    _ => false
                };

                if again {
                    self.read(scope, s)
                } else {
                    s
                }
            },
            State::Closed => {
                trace!("on_readable State::Closed");
                State::Closed
            }
        }
    }

    fn write<F: MessageHandlerFactory<K, T, Output=H>>(&mut self, scope: &mut Scope<F>, mut state: State<H, T>) -> State<H, T> {
        let next = match state {
            State::Init { interest: Next_::Write, .. } => {
                // this is a Client request, which writes first, so pay
                // attention to the version written here, which will adjust
                // our internal state to Http1 or Http2
                let mut handler = match scope.create(Seed(&self.key, &self.ctrl.0)) {
                    Some(handler) => handler,
                    None => {
                        trace!("could not create handler {:?}", self.key);
                        return State::Closed;
                    }
                };
                let mut head = http::MessageHead::default();
                let mut interest = handler.on_outgoing(&mut head);
                if head.version == HttpVersion::Http11 {
                    let mut buf = Vec::new();
                    let keep_alive = self.keep_alive_enabled && head.should_keep_alive();
                    let mut encoder = H::Message::encode(head, &mut buf);
                    let writing = match interest.interest {
                        // user wants to write some data right away
                        // try to write the headers and the first chunk
                        // together, so they are in the same packet
                        Next_::Write |
                        Next_::ReadWrite => {
                            encoder.prefix(WriteBuf {
                                bytes: buf,
                                pos: 0
                            });
                            interest = handler.on_encode(&mut Encoder::h1(&mut encoder, &mut self.transport));
                            Writing::Ready(encoder)
                        },
                        _ => Writing::Chunk(Chunk {
                            buf: Cow::Owned(buf),
                            pos: 0,
                            next: (encoder, interest.clone())
                        })
                    };
                    state = State::Http1(Http1 {
                        reading: Reading::Init,
                        writing: writing,
                        handler: handler,
                        keep_alive: keep_alive,
                        timeout: interest.timeout,
                        timeout_start: Some(scope.now()),
                        _marker: PhantomData,
                    })
                }
                Some(interest)
            }
            State::Init { .. } => {
                trace!("Conn.on_writable State::{:?}", state);
                None
            }
            State::Http1(Http1 { ref mut handler, ref mut writing, ref mut keep_alive, .. }) => {
                match *writing {
                    Writing::Init => {
                        trace!("Conn.on_writable Http1::Writing::Init");
                        None
                    }
                    Writing::Head => {
                        let mut head = http::MessageHead::default();
                        let mut interest = handler.on_outgoing(&mut head);
                        // if the request wants to close, server cannot stop it
                        if *keep_alive {
                            // if the request wants to stay alive, then it depends
                            // on the server to agree
                            *keep_alive = head.should_keep_alive();
                        }
                        let mut buf = Vec::new();
                        let mut encoder = <<H as MessageHandler<T>>::Message as Http1Message>::encode(head, &mut buf);
                        *writing = match interest.interest {
                            // user wants to write some data right away
                            // try to write the headers and the first chunk
                            // together, so they are in the same packet
                            Next_::Write |
                            Next_::ReadWrite => {
                                encoder.prefix(WriteBuf {
                                    bytes: buf,
                                    pos: 0
                                });
                                interest = handler.on_encode(&mut Encoder::h1(&mut encoder, &mut self.transport));
                                Writing::Ready(encoder)
                            },
                            _ => Writing::Chunk(Chunk {
                                buf: Cow::Owned(buf),
                                pos: 0,
                                next: (encoder, interest.clone())
                            })
                        };
                        Some(interest)
                    },
                    Writing::Chunk(ref mut chunk) => {
                        trace!("Http1.Chunk on_writable");
                        match self.transport.write(&chunk.buf.as_ref()[chunk.pos..]) {
                            Ok(n) => {
                                chunk.pos += n;
                                trace!("Http1.Chunk wrote={}, done={}", n, chunk.is_written());
                                if chunk.is_written() {
                                    Some(chunk.next.1.clone())
                                } else {
                                    None
                                }
                            },
                            Err(e) => match e.kind() {
                                io::ErrorKind::WouldBlock |
                                io::ErrorKind::Interrupted => None,
                                _ => {
                                    Some(handler.on_error(e.into()))
                                }
                            }
                        }
                    },
                    Writing::Ready(ref mut encoder) => {
                        trace!("Http1.Ready on_writable");
                        Some(handler.on_encode(&mut Encoder::h1(encoder, &mut self.transport)))
                    },
                    Writing::Wait(..) => {
                        trace!("Conn.on_writable Http1::Writing::Wait");
                        None
                    }
                    Writing::KeepAlive => {
                        trace!("Conn.on_writable Http1::Writing::KeepAlive");
                        None
                    }
                    Writing::Closed => {
                        trace!("on_writable Http1::Writing::Closed");
                        None
                    }
                }
            },
            State::Closed => {
                trace!("on_writable State::Closed");
                None
            }
        };

        if let Some(next) = next {
            state.update(next, &**scope, Some(scope.now()));
        }
        state
    }

    fn can_read_more(&self, was_init: bool) -> bool {
        match self.state {
            State::Init { .. } => !was_init && !self.buf.is_empty(),
            _ => !self.buf.is_empty()
        }
    }

    fn on_error<F>(&mut self, err: ::Error, factory: &F) where F: MessageHandlerFactory<K, T> {
        debug!("on_error err = {:?}", err);
        trace!("on_error state = {:?}", self.state);
        let next = match self.state {
            State::Init { .. } => Next::remove(),
            State::Http1(ref mut http1) => http1.handler.on_error(err),
            State::Closed => Next::remove(),
        };
        self.state.update(next, factory, None);
    }

    fn on_readable<F>(&mut self, scope: &mut Scope<F>)
    where F: MessageHandlerFactory<K, T, Output=H> {
        trace!("on_readable -> {:?}", self.state);
        let state = mem::replace(&mut self.state, State::Closed);
        self.state = self.read(scope, state);
        trace!("on_readable <- {:?}", self.state);
    }

    fn on_writable<F>(&mut self, scope: &mut Scope<F>)
    where F: MessageHandlerFactory<K, T, Output=H> {
        trace!("on_writable -> {:?}", self.state);
        let state = mem::replace(&mut self.state, State::Closed);
        self.state = self.write(scope, state);
        trace!("on_writable <- {:?}", self.state);
    }

    fn on_remove(self) {
        debug!("on_remove");
        match self.state {
            State::Init { .. } | State::Closed => (),
            State::Http1(http1) => http1.handler.on_remove(self.transport),
        }
    }

}

pub enum ReadyResult<C> {
    Continue(C),
    Done(Option<(C, Option<Duration>)>)
}

impl<K: Key, T: Transport, H: MessageHandler<T>> Conn<K, T, H> {
    pub fn new(
        key: K,
        transport: T,
        next: Next,
        notify: rotor::Notifier,
        now: Time
    ) -> Conn<K, T, H> {
        Conn(Box::new(ConnInner {
            buf: Buffer::new(),
            ctrl: channel::new(notify),
            keep_alive_enabled: true,
            key: key,
            state: State::Init {
                interest: next.interest,
                timeout: next.timeout,
                timeout_start: Some(now),
            },
            transport: transport,
        }))
    }

    pub fn keep_alive(mut self, val: bool) -> Conn<K, T, H> {
        self.0.keep_alive_enabled = val;
        self
    }

    pub fn ready<F>(
        mut self,
        events: EventSet,
        scope: &mut Scope<F>
    ) -> ReadyResult<Self>
        where F: MessageHandlerFactory<K, T, Output=H>
    {
        trace!("Conn::ready events='{:?}', blocked={:?}", events, self.0.transport.blocked());

        if events.is_error() {
            match self.0.transport.take_socket_error() {
                Ok(_) => {
                    trace!("is_error, but not socket error");
                    // spurious?
                },
                Err(e) => self.0.on_error(e.into(), &**scope)
            }
        }

        if events.is_hup() {
            trace!("Conn::ready got hangup");
            let _ = scope.deregister(&self.0.transport);
            self.on_remove();
            return ReadyResult::Done(None);
        }

        // if the user had an io interest, but the transport was blocked differently,
        // the event needs to be translated to what the user was actually expecting.
        //
        // Example:
        // - User asks for `Next::write().
        // - But transport is in the middle of renegotiating TLS, and is blocked on reading.
        // - hyper should not wait on the `write` event, since epoll already
        //   knows it is writable. We would just loop a whole bunch, and slow down.
        // - So instead, hyper waits on the event needed to unblock the transport, `read`.
        // - Once epoll detects the transport is readable, it will alert hyper
        //   with a `readable` event.
        // - hyper needs to translate that `readable` event back into a `write`,
        //   since that is actually what the Handler wants.

        let events = if let Some(blocked) = self.0.transport.blocked() {
            let interest = self.0.interest();
            trace!("translating blocked={:?}, interest={:?}", blocked, interest);
            match (blocked, interest) {
                (Blocked::Read, Reg::Write) => EventSet::writable(),
                (Blocked::Write, Reg::Read) => EventSet::readable(),
                // otherwise, the transport was blocked on the same thing the user wanted
                _ => events
            }
        } else {
            events
        };

        let was_init = match self.0.state {
            State::Init { .. } => true,
            _ => false
        };

        if events.is_readable() {
            self.0.on_readable(scope);
        }

        if events.is_writable() {
            self.0.on_writable(scope);
        }

        let mut events = match self.0.register() {
            Reg::Read => EventSet::readable(),
            Reg::Write => EventSet::writable(),
            Reg::ReadWrite => EventSet::readable() | EventSet::writable(),
            Reg::Wait => EventSet::none(),
            Reg::Remove => {
                trace!("removing transport");
                let _ = scope.deregister(&self.0.transport);
                self.on_remove();
                return ReadyResult::Done(None);
            },
        };

        if events.is_readable() && self.0.can_read_more(was_init) {
            return ReadyResult::Continue(self);
        }

        events = events | EventSet::hup();

        trace!("scope.reregister({:?})", events);
        match scope.reregister(&self.0.transport, events, PollOpt::level()) {
            Ok(..) => {
                let timeout = self.0.state.timeout();
                ReadyResult::Done(Some((self, timeout)))
            },
            Err(e) => {
                trace!("error reregistering: {:?}", e);
                self.0.on_error(e.into(), &**scope);
                ReadyResult::Done(None)
            }
        }
    }

    pub fn wakeup<F>(mut self, scope: &mut Scope<F>) -> Option<(Self, Option<Duration>)>
    where F: MessageHandlerFactory<K, T, Output=H> {
        while let Ok(next) = self.0.ctrl.1.try_recv() {
            trace!("woke up with {:?}", next);
            let timeout_start = self.0.state.timeout_start();
            self.0.state.update(next, &**scope, timeout_start);
        }

        let mut conn = Some(self);
        loop {
            match conn.take().unwrap().ready(EventSet::readable() | EventSet::writable(), scope) {
                ReadyResult::Done(val) => return val,
                ReadyResult::Continue(c) => conn = Some(c),
            }
        }
    }

    pub fn timeout<F>(mut self, scope: &mut Scope<F>) -> Option<(Self, Option<Duration>)>
    where F: MessageHandlerFactory<K, T, Output=H> {
        // Run error handler if timeout has elapsed
        if self.0.state.timeout_elapsed(scope.now()) {
            self.0.on_error(::Error::Timeout, &**scope);
        }

        let mut conn = Some(self);
        loop {
            match conn.take().unwrap().ready(EventSet::none(), scope) {
                ReadyResult::Done(val) => return val,
                ReadyResult::Continue(c) => conn = Some(c),
            }
        }
    }

    fn on_remove(self) {
        self.0.on_remove()
    }

    pub fn key(&self) -> &K {
        &self.0.key
    }

    pub fn control(&self) -> Control {
        Control {
            tx: self.0.ctrl.0.clone(),
        }
    }

    pub fn is_idle(&self) -> bool {
        if let State::Init { interest: Next_::Wait, .. } = self.0.state {
            true
        } else {
            false
        }
    }
}

enum State<H: MessageHandler<T>, T: Transport> {
    Init {
        interest: Next_,
        timeout: Option<Duration>,
        timeout_start: Option<Time>,
    },
    /// Http1 will only ever use a connection to send and receive a single
    /// message at a time. Once a H1 status has been determined, we will either
    /// be reading or writing an H1 message, and optionally multiple if
    /// keep-alive is true.
    Http1(Http1<H, T>),
    /// Http2 allows multiplexing streams over a single connection. So even
    /// when we've identified a certain message, we must always parse frame
    /// head to determine if the incoming frame is part of a current message,
    /// or a new one. This also means we could have multiple messages at once.
    //Http2 {},
    Closed,
}

/// Given two rotor::Time and a duration, see if the duration has elapsed.
///
/// The rotor::Time type only implements Add<Duration>, doesn't provide an API for comparing
/// itself with other rotor::Time, and it doesn't implement arithmetic operations with itself.
///
/// `Time` is just a newtype around (u64). Since there's no other way to compare them, we'll just
/// use this knowledge to actually do a comparison.
fn timeout_elapsed(timeout: Duration, start: Time, now: Time) -> bool {
    // type annotation for sanity
    let timeout_at: rotor::Time = start + timeout;

    let timeout_at: u64 = unsafe { mem::transmute(timeout_at) };
    let now: u64 = unsafe { mem::transmute(now) };

    if now >= timeout_at {
        true
    } else {
        false
    }
}


impl<H: MessageHandler<T>, T: Transport> State<H, T> {
    fn timeout(&self) -> Option<Duration> {
        match *self {
            State::Init { timeout, .. } => timeout,
            State::Http1(ref http1) => http1.timeout,
            State::Closed => None,
        }
    }

    fn timeout_start(&self) -> Option<Time> {
        match *self {
            State::Init { timeout_start, .. } => timeout_start,
            State::Http1(ref http1) => http1.timeout_start,
            State::Closed => None,
        }
    }

    fn timeout_elapsed(&self, now: Time) -> bool {
        match *self {
            State::Init { timeout, timeout_start, .. } => {
                if let (Some(timeout), Some(start)) = (timeout, timeout_start) {
                    timeout_elapsed(timeout, start, now)
                } else {
                    false
                }
            },
            State::Http1(ref http1) => http1.timeout_elapsed(now),
            State::Closed => false,
        }
    }
}

impl<H: MessageHandler<T>, T: Transport> fmt::Debug for State<H, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            State::Init { interest, timeout, timeout_start } => f.debug_struct("Init")
                .field("interest", &interest)
                .field("timeout", &timeout)
                .field("timeout_start", &timeout_start)
                .finish(),
            State::Http1(ref h1) => f.debug_tuple("Http1")
                .field(h1)
                .finish(),
            State::Closed => f.write_str("Closed")
        }
    }
}

impl<H: MessageHandler<T>, T: Transport> State<H, T> {
    fn update<F, K>(&mut self, next: Next, factory: &F, timeout_start: Option<Time>)
            where F: MessageHandlerFactory<K, T>,
                  K: Key
        {
            let timeout = next.timeout;
            let state = mem::replace(self, State::Closed);
            trace!("State::update state={:?}, interest={:?}", state, next.interest);
            match (state, next.interest) {
                (_, Next_::Remove) |
                (State::Closed, _) => return, // Keep State::Closed.
                (State::Init { .. }, e) => {
                    mem::replace(self,
                                 State::Init {
                                     interest: e,
                                     timeout: timeout,
                                     timeout_start: timeout_start,
                                 });
                }
                (State::Http1(mut http1), next_) => {
                    match next_ {
                        Next_::Remove => unreachable!(), // Covered in (_, Next_::Remove) case above.
                        Next_::End => {
                            let reading = match http1.reading {
                                Reading::Body(ref decoder) |
                                Reading::Wait(ref decoder) if decoder.is_eof() => {
                                    if http1.keep_alive {
                                        Reading::KeepAlive
                                    } else {
                                        Reading::Closed
                                    }
                                }
                                Reading::KeepAlive => http1.reading,
                                _ => Reading::Closed,
                            };
                            let mut writing = Writing::Closed;
                            let encoder = match http1.writing {
                                Writing::Wait(enc) |
                                Writing::Ready(enc) => Some(enc),
                                Writing::Chunk(mut chunk) => {
                                    if chunk.is_written() {
                                        Some(chunk.next.0)
                                    } else {
                                        chunk.next.1 = next;
                                        writing = Writing::Chunk(chunk);
                                        None
                                    }
                                }
                                Writing::KeepAlive => {
                                    writing = Writing::KeepAlive;
                                    None
                                }
                                _ => return, // Keep State::Closed.
                            };
                            if let Some(encoder) = encoder {
                                if encoder.is_eof() {
                                    if http1.keep_alive {
                                        writing = Writing::KeepAlive
                                    }
                                } else if let Some(buf) = encoder.finish() {
                                    writing = Writing::Chunk(Chunk {
                                        buf: buf.bytes,
                                        pos: buf.pos,
                                        next: (h1::Encoder::length(0), Next::end()),
                                    })
                                }
                            };

                            trace!("(reading, writing) -> {:?}", (&reading, &writing));
                            match (reading, writing) {
                                (Reading::KeepAlive, Writing::KeepAlive) => {
                                    let next = factory.keep_alive_interest();
                                    mem::replace(self,
                                                 State::Init {
                                                     interest: next.interest,
                                                     timeout: next.timeout,
                                                     timeout_start: timeout_start,
                                                 });
                                    return;
                                }
                                (reading, Writing::Chunk(chunk)) => {
                                    http1.reading = reading;
                                    http1.writing = Writing::Chunk(chunk);
                                }
                                _ => return, // Keep State::Closed.
                            }
                        }
                        Next_::Read => {
                            http1.reading = match http1.reading {
                                Reading::Init => Reading::Parse,
                                Reading::Wait(decoder) => Reading::Body(decoder),
                                same => same,
                            };

                            http1.writing = match http1.writing {
                                Writing::Ready(encoder) => {
                                    if encoder.is_eof() {
                                        if http1.keep_alive {
                                            Writing::KeepAlive
                                        } else {
                                            Writing::Closed
                                        }
                                    } else if encoder.is_closed() {
                                        if let Some(buf) = encoder.finish() {
                                            Writing::Chunk(Chunk {
                                                buf: buf.bytes,
                                                pos: buf.pos,
                                                next: (h1::Encoder::length(0), Next::wait()),
                                            })
                                        } else {
                                            Writing::Closed
                                        }
                                    } else {
                                        Writing::Wait(encoder)
                                    }
                                }
                                Writing::Chunk(chunk) => {
                                    if chunk.is_written() {
                                        Writing::Wait(chunk.next.0)
                                    } else {
                                        Writing::Chunk(chunk)
                                    }
                                }
                                same => same,
                            };
                        }
                        Next_::Write => {
                            http1.writing = match http1.writing {
                                Writing::Wait(encoder) => Writing::Ready(encoder),
                                Writing::Init => Writing::Head,
                                Writing::Chunk(chunk) => {
                                    if chunk.is_written() {
                                        Writing::Ready(chunk.next.0)
                                    } else {
                                        Writing::Chunk(chunk)
                                    }
                                }
                                same => same,
                            };

                            http1.reading = match http1.reading {
                                Reading::Body(decoder) => {
                                    if decoder.is_eof() {
                                        if http1.keep_alive {
                                            Reading::KeepAlive
                                        } else {
                                            Reading::Closed
                                        }
                                    } else {
                                        Reading::Wait(decoder)
                                    }
                                }
                                same => same,
                            };
                        }
                        Next_::ReadWrite => {
                            http1.reading = match http1.reading {
                                Reading::Init => Reading::Parse,
                                Reading::Wait(decoder) => Reading::Body(decoder),
                                same => same,
                            };
                            http1.writing = match http1.writing {
                                Writing::Wait(encoder) => Writing::Ready(encoder),
                                Writing::Init => Writing::Head,
                                Writing::Chunk(chunk) => {
                                    if chunk.is_written() {
                                        Writing::Ready(chunk.next.0)
                                    } else {
                                        Writing::Chunk(chunk)
                                    }
                                }
                                same => same,
                            };
                        }
                        Next_::Wait => {
                            http1.reading = match http1.reading {
                                Reading::Body(decoder) => Reading::Wait(decoder),
                                same => same,
                            };

                            http1.writing = match http1.writing {
                                Writing::Ready(encoder) => Writing::Wait(encoder),
                                Writing::Chunk(chunk) => {
                                    if chunk.is_written() {
                                        Writing::Wait(chunk.next.0)
                                    } else {
                                        Writing::Chunk(chunk)
                                    }
                                }
                                same => same,
                            };
                        }
                    }
                    http1.timeout = timeout;
                    mem::replace(self, State::Http1(http1));
                }
            };
        }
}

// These Reading and Writing stuff should probably get moved into h1/message.rs

struct Http1<H, T> {
    handler: H,
    reading: Reading,
    writing: Writing,
    keep_alive: bool,
    timeout: Option<Duration>,
    timeout_start: Option<Time>,
    _marker: PhantomData<T>,
}

impl<H, T> Http1<H, T> {
    fn timeout_elapsed(&self, now: Time) -> bool {
        if let (Some(timeout), Some(start)) = (self.timeout, self.timeout_start) {
            timeout_elapsed(timeout, start, now)
        } else {
            false
        }
    }
}

impl<H, T> fmt::Debug for Http1<H, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Http1")
            .field("reading", &self.reading)
            .field("writing", &self.writing)
            .field("keep_alive", &self.keep_alive)
            .field("timeout", &self.timeout)
            .finish()
    }
}

#[derive(Debug)]
enum Reading {
    Init,
    Parse,
    Body(h1::Decoder),
    Wait(h1::Decoder),
    KeepAlive,
    Closed
}

#[derive(Debug)]
enum Writing {
    Init,
    Head,
    Chunk(Chunk) ,
    Ready(h1::Encoder),
    Wait(h1::Encoder),
    KeepAlive,
    Closed
}

#[derive(Debug)]
struct Chunk {
    buf: Cow<'static, [u8]>,
    pos: usize,
    next: (h1::Encoder, Next),
}

impl Chunk {
    fn is_written(&self) -> bool {
        self.pos >= self.buf.len()
    }
}

pub trait MessageHandler<T: Transport> {
    type Message: Http1Message;
    fn on_incoming(&mut self, head: http::MessageHead<<Self::Message as Http1Message>::Incoming>, transport: &T) -> Next;
    fn on_outgoing(&mut self, head: &mut http::MessageHead<<Self::Message as Http1Message>::Outgoing>) -> Next;
    fn on_decode(&mut self, &mut http::Decoder<T>) -> Next;
    fn on_encode(&mut self, &mut http::Encoder<T>) -> Next;
    fn on_error(&mut self, err: ::Error) -> Next;

    fn on_remove(self, T) where Self: Sized;
}

pub struct Seed<'a, K: Key + 'a>(&'a K, &'a channel::Sender<Next>);

impl<'a, K: Key + 'a> Seed<'a, K> {
    pub fn control(&self) -> Control {
        Control {
            tx: self.1.clone(),
        }
    }

    pub fn key(&self) -> &K {
        self.0
    }
}


pub trait MessageHandlerFactory<K: Key, T: Transport> {
    type Output: MessageHandler<T>;

    fn create(&mut self, seed: Seed<K>) -> Option<Self::Output>;

    fn keep_alive_interest(&self) -> Next;
}

pub trait Key: Eq + Hash + Clone + fmt::Debug {}
impl<T: Eq + Hash + Clone + fmt::Debug> Key for T {}

#[cfg(test)]
mod tests {
    /* TODO:
    test when the underlying Transport of a Conn is blocked on an action that
    differs from the desired interest().

    Ex:
        transport.blocked() == Some(Blocked::Read)
        self.interest() == Reg::Write

        Should call `scope.register(EventSet::read())`, not with write

    #[test]
    fn test_conn_register_when_transport_blocked() {

    }
    */
}
