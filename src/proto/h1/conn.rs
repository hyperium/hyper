use std::fmt;
use std::io::{self};
use std::marker::PhantomData;

use bytes::Bytes;
use futures::{Async, AsyncSink, Poll, StartSend};
use futures::task::Task;
use http::{Method, Version};
use tokio_io::{AsyncRead, AsyncWrite};

use ::Chunk;
use proto::{BodyLength, Decode, Http1Transaction, MessageHead};
use super::io::{Cursor, Buffered};
use super::{EncodedBuf, Encoder, Decoder};


/// This handles a connection, which will have been established over an
/// `AsyncRead + AsyncWrite` (like a socket), and will likely include multiple
/// `Transaction`s over HTTP.
///
/// The connection will determine when a message begins and ends as well as
/// determine if this connection can be kept alive after the message,
/// or if it is complete.
pub(crate) struct Conn<I, B, T> {
    io: Buffered<I, EncodedBuf<Cursor<B>>>,
    state: State,
    _marker: PhantomData<T>
}

/*
impl<I, B> Conn<I, B, ClientTransaction>
where I: AsyncRead + AsyncWrite,
      B: AsRef<[u8]>,
{
    pub fn new_client(io: I) -> Conn<I, B, ClientTransaction> {
        Conn::new(io)
    }
}
*/

impl<I, B, T> Conn<I, B, T>
where I: AsyncRead + AsyncWrite,
      B: AsRef<[u8]>,
      T: Http1Transaction,
{
    pub fn new(io: I) -> Conn<I, B, T> {
        Conn {
            io: Buffered::new(io),
            state: State {
                error: None,
                keep_alive: KA::Busy,
                method: None,
                title_case_headers: false,
                read_task: None,
                reading: Reading::Init,
                writing: Writing::Init,
                // We assume a modern world where the remote speaks HTTP/1.1.
                // If they tell us otherwise, we'll downgrade in `read_head`.
                version: Version::HTTP_11,
            },
            _marker: PhantomData,
        }
    }

    pub fn set_flush_pipeline(&mut self, enabled: bool) {
        self.io.set_flush_pipeline(enabled);
    }

    pub fn set_max_buf_size(&mut self, max: usize) {
        self.io.set_max_buf_size(max);
    }

    pub fn set_write_strategy_flatten(&mut self) {
        self.io.set_write_strategy_flatten();
    }

    pub fn set_title_case_headers(&mut self) {
        self.state.title_case_headers = true;
    }

    pub fn into_inner(self) -> (I, Bytes) {
        self.io.into_inner()
    }

    pub fn is_read_closed(&self) -> bool {
        self.state.is_read_closed()
    }

    pub fn is_write_closed(&self) -> bool {
        self.state.is_write_closed()
    }

    pub fn can_read_head(&self) -> bool {
        match self.state.reading {
            //Reading::Init => true,
            Reading::Init => {
                if T::should_read_first() {
                    true
                } else {
                    match self.state.writing {
                        Writing::Init => false,
                        _ => true,
                    }
                }
            },
            _ => false,
        }
    }

    pub fn can_read_body(&self) -> bool {
        match self.state.reading {
            Reading::Body(..) => true,
            _ => false,
        }
    }

    fn should_error_on_eof(&self) -> bool {
        // If we're idle, it's probably just the connection closing gracefully.
        T::should_error_on_parse_eof() && !self.state.is_idle()
    }

    pub fn read_head(&mut self) -> Poll<Option<(MessageHead<T::Incoming>, bool)>, ::Error> {
        debug_assert!(self.can_read_head());
        trace!("Conn::read_head");

        loop {
            let (version, head) = match self.io.parse::<T>() {
                Ok(Async::Ready(head)) => (head.version, head),
                Ok(Async::NotReady) => return Ok(Async::NotReady),
                Err(e) => {
                    // If we are currently waiting on a message, then an empty
                    // message should be reported as an error. If not, it is just
                    // the connection closing gracefully.
                    let must_error = self.should_error_on_eof();
                    self.state.close_read();
                    self.io.consume_leading_lines();
                    let was_mid_parse = e.is_parse() || !self.io.read_buf().is_empty();
                    return if was_mid_parse || must_error {
                        debug!("parse error ({}) with {} bytes", e, self.io.read_buf().len());
                        self.on_parse_error(e)
                            .map(|()| Async::NotReady)
                    } else {
                        debug!("read eof");
                        Ok(Async::Ready(None))
                    };
                }
            };

            match version {
                Version::HTTP_10 |
                Version::HTTP_11 => {},
                _ => {
                    error!("unimplemented HTTP Version = {:?}", version);
                    self.state.close_read();
                    //TODO: replace this with a more descriptive error
                    return Err(::Error::new_version());
                }
            };
            self.state.version = version;

            let decoder = match T::decoder(&head, &mut self.state.method) {
                Ok(Decode::Normal(d)) => {
                    d
                },
                Ok(Decode::Final(d)) => {
                    trace!("final decoder, HTTP ending");
                    debug_assert!(d.is_eof());
                    self.state.close_read();
                    d
                },
                Ok(Decode::Ignore) => {
                    // likely a 1xx message that we can ignore
                    continue;
                }
                Err(e) => {
                    debug!("decoder error = {:?}", e);
                    self.state.close_read();
                    return self.on_parse_error(e)
                        .map(|()| Async::NotReady);
                }
            };

            debug!("incoming body is {}", decoder);

            self.state.busy();
            if head.expecting_continue() {
                let msg = b"HTTP/1.1 100 Continue\r\n\r\n";
                self.io.write_buf_mut().extend_from_slice(msg);
            }
            let wants_keep_alive = head.should_keep_alive();
            self.state.keep_alive &= wants_keep_alive;
            let (body, reading) = if decoder.is_eof() {
                (false, Reading::KeepAlive)
            } else {
                (true, Reading::Body(decoder))
            };
            if let Reading::Closed = self.state.reading {
                // actually want an `if not let ...`
            } else {
                self.state.reading = reading;
            }
            if !body {
                self.try_keep_alive();
            }
            return Ok(Async::Ready(Some((head, body))));
        }
    }

    pub fn read_body(&mut self) -> Poll<Option<Chunk>, io::Error> {
        debug_assert!(self.can_read_body());

        trace!("Conn::read_body");

        let (reading, ret) = match self.state.reading {
            Reading::Body(ref mut decoder) => {
                match decoder.decode(&mut self.io) {
                    Ok(Async::Ready(slice)) => {
                        let (reading, chunk) = if !slice.is_empty() {
                            return Ok(Async::Ready(Some(Chunk::from(slice))));
                        } else if decoder.is_eof() {
                            debug!("incoming body completed");
                            (Reading::KeepAlive, None)
                        } else {
                            trace!("decode stream unexpectedly ended");
                            // this should actually be unreachable:
                            // the decoder will return an UnexpectedEof if there were
                            // no bytes to read and it isn't eof yet...
                            (Reading::Closed, None)
                        };
                        (reading, Ok(Async::Ready(chunk)))
                    },
                    Ok(Async::NotReady) => return Ok(Async::NotReady),
                    Err(e) => {
                        trace!("decode stream error: {}", e);
                        (Reading::Closed, Err(e))
                    },
                }
            },
            _ => unreachable!("read_body invalid state: {:?}", self.state.reading),
        };

        self.state.reading = reading;
        self.try_keep_alive();
        ret
    }

    pub fn read_keep_alive(&mut self) -> Result<(), ::Error> {
        debug_assert!(!self.can_read_head() && !self.can_read_body());

        trace!("read_keep_alive; is_mid_message={}", self.is_mid_message());

        if self.is_mid_message() {
            self.maybe_park_read();
        } else {
            self.require_empty_read().map_err(::Error::new_io)?;
        }
        Ok(())
    }

    fn is_mid_message(&self) -> bool {
        match (&self.state.reading, &self.state.writing) {
            (&Reading::Init, &Writing::Init) => false,
            _ => true,
        }
    }

    fn maybe_park_read(&mut self) {
        if !self.io.is_read_blocked() {
            // the Io object is ready to read, which means it will never alert
            // us that it is ready until we drain it. However, we're currently
            // finished reading, so we need to park the task to be able to
            // wake back up later when more reading should happen.
            let park = self.state.read_task.as_ref()
                .map(|t| !t.will_notify_current())
                .unwrap_or(true);
            if park {
                trace!("parking current task");
                self.state.read_task = Some(::futures::task::current());
            }
        }
    }

    // This will check to make sure the io object read is empty.
    //
    // This should only be called for Clients wanting to enter the idle
    // state.
    fn require_empty_read(&mut self) -> io::Result<()> {
        assert!(!self.can_read_head() && !self.can_read_body());

        if !self.io.read_buf().is_empty() {
            debug!("received an unexpected {} bytes", self.io.read_buf().len());
            Err(io::Error::new(io::ErrorKind::InvalidData, "unexpected bytes after message ended"))
        } else {
            match self.try_io_read()? {
                Async::Ready(0) => {
                    // case handled in try_io_read
                    Ok(())
                },
                Async::Ready(n) => {
                    debug!("received {} bytes on an idle connection", n);
                    let desc = if self.state.is_idle() {
                        "unexpected bytes after message ended"
                    } else {
                        "unexpected bytes before writing message"
                    };
                    Err(io::Error::new(io::ErrorKind::InvalidData, desc))
                },
                Async::NotReady => {
                    Ok(())
                },
            }
        }
    }

    fn try_io_read(&mut self) -> Poll<usize, io::Error> {
         match self.io.read_from_io() {
            Ok(Async::Ready(0)) => {
                trace!("try_io_read; found EOF on connection: {:?}", self.state);
                let must_error = self.should_error_on_eof();
                let ret = if must_error {
                    let desc = if self.is_mid_message() {
                        "unexpected EOF waiting for response"
                    } else {
                        "unexpected EOF before writing message"
                    };
                    Err(io::Error::new(io::ErrorKind::UnexpectedEof, desc))
                } else {
                    Ok(Async::Ready(0))
                };

                // order is important: must_error needs state BEFORE close_read
                self.state.close_read();
                ret
            },
            Ok(Async::Ready(n)) => {
                Ok(Async::Ready(n))
            },
            Ok(Async::NotReady) => {
                Ok(Async::NotReady)
            },
            Err(e) => {
                self.state.close();
                Err(e)
            }
        }
    }


    fn maybe_notify(&mut self) {
        // its possible that we returned NotReady from poll() without having
        // exhausted the underlying Io. We would have done this when we
        // determined we couldn't keep reading until we knew how writing
        // would finish.
        //
        // When writing finishes, we need to wake the task up in case there
        // is more reading that can be done, to start a new message.



        let wants_read = match self.state.reading {
            Reading::Body(..) |
            Reading::KeepAlive => return,
            Reading::Init => true,
            Reading::Closed => false,
        };

        match self.state.writing {
            Writing::Body(..) => return,
            Writing::Init |
            Writing::KeepAlive |
            Writing::Closed => (),
        }

        if !self.io.is_read_blocked() {
            if wants_read && self.io.read_buf().is_empty() {
                match self.io.read_from_io() {
                    Ok(Async::Ready(_)) => (),
                    Ok(Async::NotReady) => {
                        trace!("maybe_notify; read_from_io blocked");
                        return
                    },
                    Err(e) => {
                        trace!("maybe_notify; read_from_io error: {}", e);
                        self.state.close();
                    }
                }
            }
            if let Some(ref task) = self.state.read_task {
                trace!("maybe_notify; notifying task");
                task.notify();
            } else {
                trace!("maybe_notify; no task to notify");
            }
        }
    }

    fn try_keep_alive(&mut self) {
        self.state.try_keep_alive();
        self.maybe_notify();
    }

    pub fn can_write_head(&self) -> bool {
        if !T::should_read_first() {
            match self.state.reading {
                Reading::Closed => return false,
                _ => {},
            }
        }
        match self.state.writing {
            Writing::Init => true,
            _ => false
        }
    }

    pub fn can_write_body(&self) -> bool {
        match self.state.writing {
            Writing::Body(..) => true,
            Writing::Init |
            Writing::KeepAlive |
            Writing::Closed => false,
        }
    }

    pub fn can_buffer_body(&self) -> bool {
        self.io.can_buffer()
    }

    pub fn write_head(&mut self, mut head: MessageHead<T::Outgoing>, body: Option<BodyLength>) {
        debug_assert!(self.can_write_head());

        if !T::should_read_first() {
            self.state.busy();
        }

        self.enforce_version(&mut head);

        let buf = self.io.write_buf_mut();
        self.state.writing = match T::encode(head, body, &mut self.state.method, self.state.title_case_headers, buf) {
            Ok(encoder) => {
                if !encoder.is_eof() {
                    Writing::Body(encoder)
                } else if encoder.is_last() {
                    Writing::Closed
                } else {
                    Writing::KeepAlive
                }
            },
            Err(err) => {
                self.state.error = Some(err);
                Writing::Closed
            }
        };
    }

    // If we know the remote speaks an older version, we try to fix up any messages
    // to work with our older peer.
    fn enforce_version(&mut self, head: &mut MessageHead<T::Outgoing>) {
        //use header::Connection;

        let wants_keep_alive = if self.state.wants_keep_alive() {
            let ka = head.should_keep_alive();
            self.state.keep_alive &= ka;
            ka
        } else {
            false
        };

        match self.state.version {
            Version::HTTP_10 => {
                // If the remote only knows HTTP/1.0, we should force ourselves
                // to do only speak HTTP/1.0 as well.
                head.version = Version::HTTP_10;
                if wants_keep_alive {
                    //TODO: head.headers.set(Connection::keep_alive());
                }
            },
            _ => {
                // If the remote speaks HTTP/1.1, then it *should* be fine with
                // both HTTP/1.0 and HTTP/1.1 from us. So again, we just let
                // the user's headers be.
            }
        }
    }

    pub fn write_body(&mut self, chunk: Option<B>) -> StartSend<Option<B>, io::Error> {
        debug_assert!(self.can_write_body());

        if !self.can_buffer_body() {
            if let Async::NotReady = self.flush()? {
                // if chunk is Some(&[]), aka empty, whatever, just skip it
                if chunk.as_ref().map(|c| c.as_ref().is_empty()).unwrap_or(false) {
                    return Ok(AsyncSink::Ready);
                } else {
                    return Ok(AsyncSink::NotReady(chunk));
                }
            }
        }

        let state = match self.state.writing {
            Writing::Body(ref mut encoder) => {
                if let Some(chunk) = chunk {
                    if chunk.as_ref().is_empty() {
                        return Ok(AsyncSink::Ready);
                    }

                    let encoded = encoder.encode(Cursor::new(chunk));
                    self.io.buffer(encoded);

                    if encoder.is_eof() {
                        if encoder.is_last() {
                            Writing::Closed
                        } else {
                            Writing::KeepAlive
                        }
                    } else {
                        return Ok(AsyncSink::Ready);
                    }
                } else {
                    // end of stream, that means we should try to eof
                    match encoder.end() {
                        Ok(end) => {
                            if let Some(end) = end {
                                self.io.buffer(end);
                            }
                            if encoder.is_last() {
                                Writing::Closed
                            } else {
                                Writing::KeepAlive
                            }
                        },
                        Err(_not_eof) => Writing::Closed,
                    }
                }
            },
            _ => unreachable!("write_body invalid state: {:?}", self.state.writing),
        };

        self.state.writing = state;
        Ok(AsyncSink::Ready)
    }

    // When we get a parse error, depending on what side we are, we might be able
    // to write a response before closing the connection.
    //
    // - Client: there is nothing we can do
    // - Server: if Response hasn't been written yet, we can send a 4xx response
    fn on_parse_error(&mut self, err: ::Error) -> ::Result<()> {
        match self.state.writing {
            Writing::Init => {
                if let Some(msg) = T::on_error(&err) {
                    self.write_head(msg, None);
                    self.state.error = Some(err);
                    return Ok(());
                }
            }
            _ => (),
        }

        // fallback is pass the error back up
        Err(err)
    }

    pub fn flush(&mut self) -> Poll<(), io::Error> {
        try_ready!(self.io.flush());
        self.try_keep_alive();
        trace!("flushed {:?}", self.state);
        Ok(Async::Ready(()))
    }

    pub fn shutdown(&mut self) -> Poll<(), io::Error> {
        match self.io.io_mut().shutdown() {
            Ok(Async::NotReady) => Ok(Async::NotReady),
            Ok(Async::Ready(())) => {
                trace!("shut down IO complete");
                Ok(Async::Ready(()))
            }
            Err(e) => {
                debug!("error shutting down IO: {}", e);
                Err(e)
            }
        }
    }

    pub fn close_read(&mut self) {
        self.state.close_read();
    }

    pub fn close_write(&mut self) {
        self.state.close_write();
    }

    pub fn disable_keep_alive(&mut self) {
        if self.state.is_idle() {
            self.state.close_read();
        } else {
            self.state.disable_keep_alive();
        }
    }

    pub fn take_error(&mut self) -> ::Result<()> {
        if let Some(err) = self.state.error.take() {
            Err(err)
        } else {
            Ok(())
        }
    }

    // Used in h1::dispatch tests
    #[cfg(test)]
    pub(super) fn io_mut(&mut self) -> &mut I {
        self.io.io_mut()
    }
}

impl<I, B: AsRef<[u8]>, T> fmt::Debug for Conn<I, B, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Conn")
            .field("state", &self.state)
            .field("io", &self.io)
            .finish()
    }
}

struct State {
    error: Option<::Error>,
    keep_alive: KA,
    method: Option<Method>,
    title_case_headers: bool,
    read_task: Option<Task>,
    reading: Reading,
    writing: Writing,
    version: Version,
}

#[derive(Debug)]
enum Reading {
    Init,
    Body(Decoder),
    KeepAlive,
    Closed,
}

enum Writing {
    Init,
    Body(Encoder),
    KeepAlive,
    Closed,
}

impl fmt::Debug for State {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("State")
            .field("reading", &self.reading)
            .field("writing", &self.writing)
            .field("keep_alive", &self.keep_alive)
            .field("error", &self.error)
            //.field("method", &self.method)
            //.field("title_case_headers", &self.title_case_headers)
            .field("read_task", &self.read_task)
            .finish()
    }
}

impl fmt::Debug for Writing {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Writing::Init => f.write_str("Init"),
            Writing::Body(ref enc) => f.debug_tuple("Body")
                .field(enc)
                .finish(),
            Writing::KeepAlive => f.write_str("KeepAlive"),
            Writing::Closed => f.write_str("Closed"),
        }
    }
}

impl ::std::ops::BitAndAssign<bool> for KA {
    fn bitand_assign(&mut self, enabled: bool) {
        if !enabled {
            *self = KA::Disabled;
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum KA {
    Idle,
    Busy,
    Disabled,
}

impl Default for KA {
    fn default() -> KA {
        KA::Busy
    }
}

impl KA {
    fn idle(&mut self) {
        *self = KA::Idle;
    }

    fn busy(&mut self) {
        *self = KA::Busy;
    }

    fn disable(&mut self) {
        *self = KA::Disabled;
    }

    fn status(&self) -> KA {
        *self
    }
}

impl State {
    fn close(&mut self) {
        trace!("State::close()");
        self.reading = Reading::Closed;
        self.writing = Writing::Closed;
        self.keep_alive.disable();
    }

    fn close_read(&mut self) {
        trace!("State::close_read()");
        self.reading = Reading::Closed;
        self.read_task = None;
        self.keep_alive.disable();
    }

    fn close_write(&mut self) {
        trace!("State::close_write()");
        self.writing = Writing::Closed;
        self.keep_alive.disable();
    }

    fn wants_keep_alive(&self) -> bool {
        if let KA::Disabled = self.keep_alive.status() {
            false
        } else {
            true
        }
    }

    fn try_keep_alive(&mut self) {
        match (&self.reading, &self.writing) {
            (&Reading::KeepAlive, &Writing::KeepAlive) => {
                if let KA::Busy = self.keep_alive.status() {
                    self.idle();
                } else {
                    self.close();
                }
            },
            (&Reading::Closed, &Writing::KeepAlive) |
            (&Reading::KeepAlive, &Writing::Closed) => {
                self.close()
            }
            _ => ()
        }
    }

    fn disable_keep_alive(&mut self) {
        self.keep_alive.disable()
    }

    fn busy(&mut self) {
        if let KA::Disabled = self.keep_alive.status() {
            return;
        }
        self.keep_alive.busy();
    }

    fn idle(&mut self) {
        self.method = None;
        self.keep_alive.idle();
        if self.is_idle() {
            self.reading = Reading::Init;
            self.writing = Writing::Init;
        } else {
            self.close();
        }
    }

    fn is_idle(&self) -> bool {
        if let KA::Idle = self.keep_alive.status() {
            true
        } else {
            false
        }
    }

    fn is_read_closed(&self) -> bool {
        match self.reading {
            Reading::Closed => true,
            _ => false
        }
    }

    fn is_write_closed(&self) -> bool {
        match self.writing {
            Writing::Closed => true,
            _ => false
        }
    }
}

#[cfg(test)]
//TODO: rewrite these using dispatch
mod tests {
    /*
    use futures::{Async, Future, Stream, Sink};
    use futures::future;

    use proto::{self, ClientTransaction, MessageHead, ServerTransaction};
    use super::super::Encoder;
    use mock::AsyncIo;

    use super::{Conn, Decoder, Reading, Writing};
    use ::uri::Uri;

    use std::str::FromStr;

    #[test]
    fn test_conn_init_read() {
        let good_message = b"GET / HTTP/1.1\r\n\r\n".to_vec();
        let len = good_message.len();
        let io = AsyncIo::new_buf(good_message, len);
        let mut conn = Conn::<_, proto::Chunk, ServerTransaction>::new(io);

        match conn.poll().unwrap() {
            Async::Ready(Some(Frame::Message { message, body: false })) => {
                assert_eq!(message, MessageHead {
                    subject: ::proto::RequestLine(::Get, Uri::from_str("/").unwrap()),
                    .. MessageHead::default()
                })
            },
            f => panic!("frame is not Frame::Message: {:?}", f)
        }
    }

    #[test]
    fn test_conn_parse_partial() {
        let _: Result<(), ()> = future::lazy(|| {
            let good_message = b"GET / HTTP/1.1\r\nHost: foo.bar\r\n\r\n".to_vec();
            let io = AsyncIo::new_buf(good_message, 10);
            let mut conn = Conn::<_, proto::Chunk, ServerTransaction>::new(io);
            assert!(conn.poll().unwrap().is_not_ready());
            conn.io.io_mut().block_in(50);
            let async = conn.poll().unwrap();
            assert!(async.is_ready());
            match async {
                Async::Ready(Some(Frame::Message { .. })) => (),
                f => panic!("frame is not Message: {:?}", f),
            }
            Ok(())
        }).wait();
    }

    #[test]
    fn test_conn_init_read_eof_idle() {
        let io = AsyncIo::new_buf(vec![], 1);
        let mut conn = Conn::<_, proto::Chunk, ServerTransaction>::new(io);
        conn.state.idle();

        match conn.poll().unwrap() {
            Async::Ready(None) => {},
            other => panic!("frame is not None: {:?}", other)
        }
    }

    #[test]
    fn test_conn_init_read_eof_idle_partial_parse() {
        let io = AsyncIo::new_buf(b"GET / HTTP/1.1".to_vec(), 100);
        let mut conn = Conn::<_, proto::Chunk, ServerTransaction>::new(io);
        conn.state.idle();

        match conn.poll() {
            Err(ref err) if err.kind() == ::std::io::ErrorKind::UnexpectedEof => {},
            other => panic!("unexpected frame: {:?}", other)
        }
    }

    #[test]
    fn test_conn_init_read_eof_busy() {
        let _: Result<(), ()> = future::lazy(|| {
            // server ignores
            let io = AsyncIo::new_eof();
            let mut conn = Conn::<_, proto::Chunk, ServerTransaction>::new(io);
            conn.state.busy();

            match conn.poll().unwrap() {
                Async::Ready(None) => {},
                other => panic!("unexpected frame: {:?}", other)
            }

            // client
            let io = AsyncIo::new_eof();
            let mut conn = Conn::<_, proto::Chunk, ClientTransaction>::new(io);
            conn.state.busy();

            match conn.poll() {
                Err(ref err) if err.kind() == ::std::io::ErrorKind::UnexpectedEof => {},
                other => panic!("unexpected frame: {:?}", other)
            }
            Ok(())
        }).wait();
    }

    #[test]
    fn test_conn_body_finish_read_eof() {
        let _: Result<(), ()> = future::lazy(|| {
            let io = AsyncIo::new_eof();
            let mut conn = Conn::<_, proto::Chunk, ClientTransaction>::new(io);
            conn.state.busy();
            conn.state.writing = Writing::KeepAlive;
            conn.state.reading = Reading::Body(Decoder::length(0));

            match conn.poll() {
                Ok(Async::Ready(Some(Frame::Body { chunk: None }))) => (),
                other => panic!("unexpected frame: {:?}", other)
            }

            // conn eofs, but tokio-proto will call poll() again, before calling flush()
            // the conn eof in this case is perfectly fine

            match conn.poll() {
                Ok(Async::Ready(None)) => (),
                other => panic!("unexpected frame: {:?}", other)
            }
            Ok(())
        }).wait();
    }

    #[test]
    fn test_conn_message_empty_body_read_eof() {
        let _: Result<(), ()> = future::lazy(|| {
            let io = AsyncIo::new_buf(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".to_vec(), 1024);
            let mut conn = Conn::<_, proto::Chunk, ClientTransaction>::new(io);
            conn.state.busy();
            conn.state.writing = Writing::KeepAlive;

            match conn.poll() {
                Ok(Async::Ready(Some(Frame::Message { body: false, .. }))) => (),
                other => panic!("unexpected frame: {:?}", other)
            }

            // conn eofs, but tokio-proto will call poll() again, before calling flush()
            // the conn eof in this case is perfectly fine

            match conn.poll() {
                Ok(Async::Ready(None)) => (),
                other => panic!("unexpected frame: {:?}", other)
            }
            Ok(())
        }).wait();
    }

    #[test]
    fn test_conn_read_body_end() {
        let _: Result<(), ()> = future::lazy(|| {
            let io = AsyncIo::new_buf(b"POST / HTTP/1.1\r\nContent-Length: 5\r\n\r\n12345".to_vec(), 1024);
            let mut conn = Conn::<_, proto::Chunk, ServerTransaction>::new(io);
            conn.state.busy();

            match conn.poll() {
                Ok(Async::Ready(Some(Frame::Message { body: true, .. }))) => (),
                other => panic!("unexpected frame: {:?}", other)
            }

            match conn.poll() {
                Ok(Async::Ready(Some(Frame::Body { chunk: Some(_) }))) => (),
                other => panic!("unexpected frame: {:?}", other)
            }

            // When the body is done, `poll` MUST return a `Body` frame with chunk set to `None`
            match conn.poll() {
                Ok(Async::Ready(Some(Frame::Body { chunk: None }))) => (),
                other => panic!("unexpected frame: {:?}", other)
            }

            match conn.poll() {
                Ok(Async::NotReady) => (),
                other => panic!("unexpected frame: {:?}", other)
            }
            Ok(())
        }).wait();
    }

    #[test]
    fn test_conn_closed_read() {
        let io = AsyncIo::new_buf(vec![], 0);
        let mut conn = Conn::<_, proto::Chunk, ServerTransaction>::new(io);
        conn.state.close();

        match conn.poll().unwrap() {
            Async::Ready(None) => {},
            other => panic!("frame is not None: {:?}", other)
        }
    }

    #[test]
    fn test_conn_body_write_length() {
        extern crate pretty_env_logger;
        let _ = pretty_env_logger::try_init();
        let _: Result<(), ()> = future::lazy(|| {
            let io = AsyncIo::new_buf(vec![], 0);
            let mut conn = Conn::<_, proto::Chunk, ServerTransaction>::new(io);
            let max = super::super::io::DEFAULT_MAX_BUFFER_SIZE + 4096;
            conn.state.writing = Writing::Body(Encoder::length((max * 2) as u64));

            assert!(conn.start_send(Frame::Body { chunk: Some(vec![b'a'; max].into()) }).unwrap().is_ready());
            assert!(!conn.can_buffer_body());

            assert!(conn.start_send(Frame::Body { chunk: Some(vec![b'b'; 1024 * 8].into()) }).unwrap().is_not_ready());

            conn.io.io_mut().block_in(1024 * 3);
            assert!(conn.poll_complete().unwrap().is_not_ready());
            conn.io.io_mut().block_in(1024 * 3);
            assert!(conn.poll_complete().unwrap().is_not_ready());
            conn.io.io_mut().block_in(max * 2);
            assert!(conn.poll_complete().unwrap().is_ready());

            assert!(conn.start_send(Frame::Body { chunk: Some(vec![b'c'; 1024 * 8].into()) }).unwrap().is_ready());
            Ok(())
        }).wait();
    }

    #[test]
    fn test_conn_body_write_chunked() {
        let _: Result<(), ()> = future::lazy(|| {
            let io = AsyncIo::new_buf(vec![], 4096);
            let mut conn = Conn::<_, proto::Chunk, ServerTransaction>::new(io);
            conn.state.writing = Writing::Body(Encoder::chunked());

            assert!(conn.start_send(Frame::Body { chunk: Some("headers".into()) }).unwrap().is_ready());
            assert!(conn.start_send(Frame::Body { chunk: Some(vec![b'x'; 8192].into()) }).unwrap().is_ready());
            Ok(())
        }).wait();
    }

    #[test]
    fn test_conn_body_flush() {
        let _: Result<(), ()> = future::lazy(|| {
            let io = AsyncIo::new_buf(vec![], 1024 * 1024 * 5);
            let mut conn = Conn::<_, proto::Chunk, ServerTransaction>::new(io);
            conn.state.writing = Writing::Body(Encoder::length(1024 * 1024));
            assert!(conn.start_send(Frame::Body { chunk: Some(vec![b'a'; 1024 * 1024].into()) }).unwrap().is_ready());
            assert!(!conn.can_buffer_body());
            conn.io.io_mut().block_in(1024 * 1024 * 5);
            assert!(conn.poll_complete().unwrap().is_ready());
            assert!(conn.can_buffer_body());
            assert!(conn.io.io_mut().flushed());

            Ok(())
        }).wait();
    }

    #[test]
    fn test_conn_parking() {
        use std::sync::Arc;
        use futures::executor::Notify;
        use futures::executor::NotifyHandle;

        struct Car {
            permit: bool,
        }
        impl Notify for Car {
            fn notify(&self, _id: usize) {
                assert!(self.permit, "unparked without permit");
            }
        }

        fn car(permit: bool) -> NotifyHandle {
            Arc::new(Car {
                permit: permit,
            }).into()
        }

        // test that once writing is done, unparks
        let f = future::lazy(|| {
            let io = AsyncIo::new_buf(vec![], 4096);
            let mut conn = Conn::<_, proto::Chunk, ServerTransaction>::new(io);
            conn.state.reading = Reading::KeepAlive;
            assert!(conn.poll().unwrap().is_not_ready());

            conn.state.writing = Writing::KeepAlive;
            assert!(conn.poll_complete().unwrap().is_ready());
            Ok::<(), ()>(())
        });
        ::futures::executor::spawn(f).poll_future_notify(&car(true), 0).unwrap();


        // test that flushing when not waiting on read doesn't unpark
        let f = future::lazy(|| {
            let io = AsyncIo::new_buf(vec![], 4096);
            let mut conn = Conn::<_, proto::Chunk, ServerTransaction>::new(io);
            conn.state.writing = Writing::KeepAlive;
            assert!(conn.poll_complete().unwrap().is_ready());
            Ok::<(), ()>(())
        });
        ::futures::executor::spawn(f).poll_future_notify(&car(false), 0).unwrap();


        // test that flushing and writing isn't done doesn't unpark
        let f = future::lazy(|| {
            let io = AsyncIo::new_buf(vec![], 4096);
            let mut conn = Conn::<_, proto::Chunk, ServerTransaction>::new(io);
            conn.state.reading = Reading::KeepAlive;
            assert!(conn.poll().unwrap().is_not_ready());
            conn.state.writing = Writing::Body(Encoder::length(5_000));
            assert!(conn.poll_complete().unwrap().is_ready());
            Ok::<(), ()>(())
        });
        ::futures::executor::spawn(f).poll_future_notify(&car(false), 0).unwrap();
    }

    #[test]
    fn test_conn_closed_write() {
        let io = AsyncIo::new_buf(vec![], 0);
        let mut conn = Conn::<_, proto::Chunk, ServerTransaction>::new(io);
        conn.state.close();

        match conn.start_send(Frame::Body { chunk: Some(b"foobar".to_vec().into()) }) {
            Err(_e) => {},
            other => panic!("did not return Err: {:?}", other)
        }

        assert!(conn.state.is_write_closed());
    }

    #[test]
    fn test_conn_write_empty_chunk() {
        let io = AsyncIo::new_buf(vec![], 0);
        let mut conn = Conn::<_, proto::Chunk, ServerTransaction>::new(io);
        conn.state.writing = Writing::KeepAlive;

        assert!(conn.start_send(Frame::Body { chunk: None }).unwrap().is_ready());
        assert!(conn.start_send(Frame::Body { chunk: Some(Vec::new().into()) }).unwrap().is_ready());
        conn.start_send(Frame::Body { chunk: Some(vec![b'a'].into()) }).unwrap_err();
    }
    */
}
