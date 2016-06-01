use std::borrow::Cow;
use std::fmt;
use std::hash::Hash;
use std::io;
use std::marker::PhantomData;
use std::mem;
use std::time::Duration;

use rotor::{self, EventSet, PollOpt, Scope};

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
pub struct Conn<K: Key, T: Transport, H: MessageHandler<T>> {
    buf: Buffer,
    ctrl: (channel::Sender<Next>, channel::Receiver<Next>),
    keep_alive_enabled: bool,
    key: K,
    state: State<H, T>,
    transport: T,
}

impl<K: Key, T: Transport, H: MessageHandler<T>> fmt::Debug for Conn<K, T, H> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Conn")
            .field("keep_alive_enabled", &self.keep_alive_enabled)
            .field("state", &self.state)
            .field("buf", &self.buf)
            .finish()
    }
}

impl<K: Key, T: Transport, H: MessageHandler<T>> Conn<K, T, H> {
    pub fn new(key: K, transport: T, notify: rotor::Notifier) -> Conn<K, T, H> {
        Conn {
            buf: Buffer::new(),
            ctrl: channel::new(notify),
            keep_alive_enabled: true,
            key: key,
            state: State::Init,
            transport: transport,
        }
    }

    pub fn keep_alive(mut self, val: bool) -> Conn<K, T, H> {
        self.keep_alive_enabled = val;
        self
    }

    /// Desired Register interest based on state of current connection.
    ///
    /// This includes the user interest, such as when they return `Next::read()`.
    fn interest(&self) -> Reg {
        match self.state {
            State::Closed => Reg::Remove,
            State::Init => {
                <H as MessageHandler>::Message::initial_interest().interest()
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
        let n = try!(self.buf.read_from(&mut self.transport));
        if n == 0 {
            trace!("parse eof");
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "parse eof").into());
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
            State::Init => {
                let head = match self.parse() {
                    Ok(head) => head,
                    Err(::Error::Io(e)) => match e.kind() {
                        io::ErrorKind::WouldBlock |
                        io::ErrorKind::Interrupted => return State::Init,
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
                match <<H as MessageHandler<T>>::Message as Http1Message>::decoder(&head) {
                    Ok(decoder) => {
                        trace!("decoder = {:?}", decoder);
                        let keep_alive = self.keep_alive_enabled && head.should_keep_alive();
                        let mut handler = scope.create(Seed(&self.key, &self.ctrl.0));
                        let next = handler.on_incoming(head);
                        trace!("handler.on_incoming() -> {:?}", next);

                        match next.interest {
                            Next_::Read => self.read(scope, State::Http1(Http1 {
                                handler: handler,
                                reading: Reading::Body(decoder),
                                writing: Writing::Init,
                                keep_alive: keep_alive,
                                timeout: next.timeout,
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
                                _marker: PhantomData,
                            }),
                            Next_::ReadWrite => self.read(scope, State::Http1(Http1 {
                                handler: handler,
                                reading: Reading::Body(decoder),
                                writing: Writing::Head,
                                keep_alive: keep_alive,
                                timeout: next.timeout,
                                _marker: PhantomData,
                            })),
                            Next_::Wait => State::Http1(Http1 {
                                handler: handler,
                                reading: Reading::Wait(decoder),
                                writing: Writing::Init,
                                keep_alive: keep_alive,
                                timeout: next.timeout,
                                _marker: PhantomData,
                            }),
                            Next_::End |
                            Next_::Remove => State::Closed,
                        }
                    },
                    Err(e) => {
                        debug!("error creating decoder: {:?}", e);
                        //TODO: respond with 400
                        State::Closed
                    }
                }
            },
            State::Http1(mut http1) => {
                let next = match http1.reading {
                    Reading::Init => None,
                    Reading::Parse => match self.parse() {
                        Ok(head) => match <<H as MessageHandler<T>>::Message as Http1Message>::decoder(&head) {
                            Ok(decoder) => {
                                trace!("decoder = {:?}", decoder);
                                // if client request asked for keep alive,
                                // then it depends entirely on if the server agreed
                                if http1.keep_alive {
                                    http1.keep_alive = head.should_keep_alive();
                                }
                                let next = http1.handler.on_incoming(head);
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
                            //TODO: send proper error codes depending on error
                            trace!("parse error: {:?}", e);
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
                    s.update(next);
                }
                trace!("Conn.on_readable State::Http1 completed, new state = {:?}", s);

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
            State::Init => {
                // this could be a Client request, which writes first, so pay
                // attention to the version written here, which will adjust
                // our internal state to Http1 or Http2
                let mut handler = scope.create(Seed(&self.key, &self.ctrl.0));
                let mut head = http::MessageHead::default();
                let interest = handler.on_outgoing(&mut head);
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
                        _marker: PhantomData,
                    })
                }
                Some(interest)
            }
            State::Http1(Http1 { ref mut handler, ref mut writing, ref mut keep_alive, .. }) => {
                match *writing {
                    Writing::Init => {
                        trace!("Conn.on_writable Http1::Writing::Init");
                        None
                    }
                    Writing::Head => {
                        let mut head = http::MessageHead::default();
                        let interest = handler.on_outgoing(&mut head);
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
            state.update(next);
        }
        state
    }

    fn can_read_more(&self) -> bool {
        match self.state {
            State::Init => false,
            _ => !self.buf.is_empty()
        }
    }

    pub fn ready<F>(mut self, events: EventSet, scope: &mut Scope<F>) -> Option<(Self, Option<Duration>)>
    where F: MessageHandlerFactory<K, T, Output=H> {
        trace!("Conn::ready events='{:?}', blocked={:?}", events, self.transport.blocked());

        if events.is_error() {
            match self.transport.take_socket_error() {
                Ok(_) => {
                    trace!("is_error, but not socket error");
                    // spurious?
                },
                Err(e) => self.on_error(e.into())
            }
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

        let events = if let Some(blocked) = self.transport.blocked() {
            let interest = self.interest();
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

        if events.is_readable() {
            self.on_readable(scope);
        }

        if events.is_writable() {
            self.on_writable(scope);
        }

        let events = match self.register() {
            Reg::Read => EventSet::readable(),
            Reg::Write => EventSet::writable(),
            Reg::ReadWrite => EventSet::readable() | EventSet::writable(),
            Reg::Wait => EventSet::none(),
            Reg::Remove => {
                trace!("removing transport");
                let _ = scope.deregister(&self.transport);
                self.on_remove();
                return None;
            },
        };

        if events.is_readable() && self.can_read_more() {
            return self.ready(events, scope);
        }

        trace!("scope.reregister({:?})", events);
        match scope.reregister(&self.transport, events, PollOpt::level()) {
            Ok(..) => {
                let timeout = self.state.timeout();
                Some((self, timeout))
            },
            Err(e) => {
                trace!("error reregistering: {:?}", e);
                let _ = self.on_error(e.into());
                None
            }
        }
    }

    pub fn wakeup<F>(mut self, scope: &mut Scope<F>) -> Option<(Self, Option<Duration>)>
    where F: MessageHandlerFactory<K, T, Output=H> {
        loop {
            match self.ctrl.1.try_recv() {
                Ok(next) => {
                    trace!("woke up with {:?}", next);
                    self.state.update(next);
                },
                Err(_) => break
            }
        }
        self.ready(EventSet::readable() | EventSet::writable(), scope)
    }

    pub fn timeout<F>(mut self, scope: &mut Scope<F>) -> Option<(Self, Option<Duration>)>
    where F: MessageHandlerFactory<K, T, Output=H> {
        //TODO: check if this was a spurious timeout?
        self.on_error(::Error::Timeout);
        self.ready(EventSet::none(), scope)
    }

    fn on_error(&mut self, err: ::Error) {
        debug!("on_error err = {:?}", err);
        trace!("on_error state = {:?}", self.state);
        let next = match self.state {
            State::Init => Next::remove(),
            State::Http1(ref mut http1) => http1.handler.on_error(err),
            State::Closed => Next::remove(),
        };
        self.state.update(next);
    }

    fn on_remove(self) {
        debug!("on_remove");
        match self.state {
            State::Init | State::Closed => (),
            State::Http1(http1) => http1.handler.on_remove(self.transport),
        }
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
}

enum State<H: MessageHandler<T>, T: Transport> {
    Init,
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


impl<H: MessageHandler<T>, T: Transport> State<H, T> {
    fn timeout(&self) -> Option<Duration> {
        match *self {
            State::Init => None,
            State::Http1(ref http1) => http1.timeout,
            State::Closed => None,
        }
    }
}

impl<H: MessageHandler<T>, T: Transport> fmt::Debug for State<H, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            State::Init => f.write_str("Init"),
            State::Http1(ref h1) => f.debug_tuple("Http1")
                .field(h1)
                .finish(),
            State::Closed => f.write_str("Closed")
        }
    }
}

impl<H: MessageHandler<T>, T: Transport> State<H, T> {
    fn update(&mut self, next: Next) {
        let timeout = next.timeout;
        let state = mem::replace(self, State::Closed);
        let new_state = match (state, next.interest) {
            (_, Next_::Remove) => State::Closed,
            (State::Closed, _) => State::Closed,
            (State::Init, _) => State::Init,
            (State::Http1(http1), Next_::End) => {
                let reading = match http1.reading {
                    Reading::Body(ref decoder) if decoder.is_eof() => {
                        if http1.keep_alive {
                            Reading::KeepAlive
                        } else {
                            Reading::Closed
                        }
                    },
                    Reading::KeepAlive => http1.reading,
                    _ => Reading::Closed,
                };
                let writing = match http1.writing {
                    Writing::Ready(ref encoder) if encoder.is_eof() => {
                        if http1.keep_alive {
                            Writing::KeepAlive
                        } else {
                            Writing::Closed
                        }
                    },
                    Writing::Ready(encoder) => {
                        if encoder.is_eof() {
                            if http1.keep_alive {
                                Writing::KeepAlive
                            } else {
                                Writing::Closed
                            }
                        } else if let Some(buf) = encoder.end() {
                            Writing::Chunk(Chunk {
                                buf: buf.bytes,
                                pos: buf.pos,
                                next: (h1::Encoder::length(0), Next::end())
                            })
                        } else {
                            Writing::Closed
                        }
                    }
                    Writing::Chunk(mut chunk) => {
                        if chunk.is_written() {
                            let encoder = chunk.next.0;
                            //TODO: de-dupe this code and from  Writing::Ready
                            if encoder.is_eof() {
                                if http1.keep_alive {
                                    Writing::KeepAlive
                                } else {
                                    Writing::Closed
                                }
                            } else if let Some(buf) = encoder.end() {
                                Writing::Chunk(Chunk {
                                    buf: buf.bytes,
                                    pos: buf.pos,
                                    next: (h1::Encoder::length(0), Next::end())
                                })
                            } else {
                                Writing::Closed
                            }
                        } else {
                            chunk.next.1 = next;
                            Writing::Chunk(chunk)
                        }
                    },
                    _ => Writing::Closed,
                };
                match (reading, writing) {
                    (Reading::KeepAlive, Writing::KeepAlive) => {
                        //http1.handler.on_keep_alive();
                        State::Init
                    },
                    (reading, Writing::Chunk(chunk)) => {
                        State::Http1(Http1 {
                            reading: reading,
                            writing: Writing::Chunk(chunk),
                            .. http1
                        })
                    }
                    _ => State::Closed
                }
            },
            (State::Http1(mut http1), Next_::Read) => {
                http1.reading = match http1.reading {
                    Reading::Init => Reading::Parse,
                    Reading::Wait(decoder) => Reading::Body(decoder),
                    same => same
                };

                http1.writing = match http1.writing {
                    Writing::Ready(encoder) => if encoder.is_eof() {
                        if http1.keep_alive {
                            Writing::KeepAlive
                        } else {
                            Writing::Closed
                        }
                    } else {
                        Writing::Wait(encoder)
                    },
                    Writing::Chunk(chunk) => if chunk.is_written() {
                        Writing::Wait(chunk.next.0)
                    } else {
                        Writing::Chunk(chunk)
                    },
                    same => same
                };

                State::Http1(http1)
            },
            (State::Http1(mut http1), Next_::Write) => {
                http1.writing = match http1.writing {
                    Writing::Wait(encoder) => Writing::Ready(encoder),
                    Writing::Init => Writing::Head,
                    Writing::Chunk(chunk) => if chunk.is_written() {
                        Writing::Ready(chunk.next.0)
                    } else {
                        Writing::Chunk(chunk)
                    },
                    same => same
                };

                http1.reading = match http1.reading {
                    Reading::Body(decoder) => if decoder.is_eof() {
                        if http1.keep_alive {
                            Reading::KeepAlive
                        } else {
                            Reading::Closed
                        }
                    } else {
                        Reading::Wait(decoder)
                    },
                    same => same
                };
                State::Http1(http1)
            },
            (State::Http1(mut http1), Next_::ReadWrite) => {
                http1.reading = match http1.reading {
                    Reading::Init => Reading::Parse,
                    Reading::Wait(decoder) => Reading::Body(decoder),
                    same => same
                };
                http1.writing = match http1.writing {
                    Writing::Wait(encoder) => Writing::Ready(encoder),
                    Writing::Init => Writing::Head,
                    Writing::Chunk(chunk) => if chunk.is_written() {
                        Writing::Ready(chunk.next.0)
                    } else {
                        Writing::Chunk(chunk)
                    },
                    same => same
                };
                State::Http1(http1)
            },
            (State::Http1(mut http1), Next_::Wait) => {
                http1.reading = match http1.reading {
                    Reading::Body(decoder) => Reading::Wait(decoder),
                    same => same
                };

                http1.writing = match http1.writing {
                    Writing::Ready(encoder) => Writing::Wait(encoder),
                    Writing::Chunk(chunk) => if chunk.is_written() {
                        Writing::Wait(chunk.next.0)
                    } else {
                        Writing::Chunk(chunk)
                    },
                    same => same
                };
                State::Http1(http1)
            }
        };
        let new_state = match new_state {
            State::Http1(mut http1) => {
                http1.timeout = timeout;
                State::Http1(http1)
            }
            other => other
        };
        mem::replace(self, new_state);
    }
}

// These Reading and Writing stuff should probably get moved into h1/message.rs

struct Http1<H, T> {
    handler: H,
    reading: Reading,
    writing: Writing,
    keep_alive: bool,
    timeout: Option<Duration>,
    _marker: PhantomData<T>,
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
    fn on_incoming(&mut self, head: http::MessageHead<<Self::Message as Http1Message>::Incoming>) -> Next;
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
        &self.0
    }
}


pub trait MessageHandlerFactory<K: Key, T: Transport> {
    type Output: MessageHandler<T>;

    fn create(&mut self, seed: Seed<K>) -> Self::Output;
}

impl<F, K, H, T> MessageHandlerFactory<K, T> for F
where F: FnMut(Seed<K>) -> H,
      K: Key,
      H: MessageHandler<T>,
      T: Transport {
    type Output = H;
    fn create(&mut self, seed: Seed<K>) -> H {
        self(seed)
    }
}

pub trait Key: Eq + Hash + Clone {}
impl<T: Eq + Hash + Clone> Key for T {}

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
