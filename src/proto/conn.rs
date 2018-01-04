use std::fmt;
use std::io::{self, Write};
use std::marker::PhantomData;

use futures::{Async, AsyncSink, Poll, StartSend};
#[cfg(feature = "tokio-proto")]
use futures::{Sink, Stream};
use futures::task::Task;
use tokio_io::{AsyncRead, AsyncWrite};
#[cfg(feature = "tokio-proto")]
use tokio_proto::streaming::pipeline::{Frame, Transport};

use proto::Http1Transaction;
use super::io::{Cursor, Buffered};
use super::h1::{Encoder, Decoder};
use method::Method;
use version::HttpVersion;


/// This handles a connection, which will have been established over an
/// `AsyncRead + AsyncWrite` (like a socket), and will likely include multiple
/// `Transaction`s over HTTP.
///
/// The connection will determine when a message begins and ends as well as
/// determine if this  connection can be kept alive after the message,
/// or if it is complete.
pub struct Conn<I, B, T, K = KA> {
    io: Buffered<I>,
    state: State<B, K>,
    _marker: PhantomData<T>
}

impl<I, B, T, K> Conn<I, B, T, K>
where I: AsyncRead + AsyncWrite,
      B: AsRef<[u8]>,
      T: Http1Transaction,
      K: KeepAlive
{
    pub fn new(io: I, keep_alive: K) -> Conn<I, B, T, K> {
        Conn {
            io: Buffered::new(io),
            state: State {
                keep_alive: keep_alive,
                method: None,
                read_task: None,
                reading: Reading::Init,
                writing: Writing::Init,
            },
            _marker: PhantomData,
        }
    }

    pub fn set_flush_pipeline(&mut self, enabled: bool) {
        self.io.set_flush_pipeline(enabled);
    }

    #[cfg(feature = "tokio-proto")]
    fn poll_incoming(&mut self) -> Poll<Option<Frame<super::MessageHead<T::Incoming>, super::Chunk, ::Error>>, io::Error> {
        trace!("Conn::poll_incoming()");

        loop {
            if self.is_read_closed() {
                trace!("Conn::poll when closed");
                return Ok(Async::Ready(None));
            } else if self.can_read_head() {
                return match self.read_head() {
                    Ok(Async::Ready(Some((head, body)))) => {
                        Ok(Async::Ready(Some(Frame::Message {
                            message: head,
                            body: body,
                        })))
                    },
                    Ok(Async::Ready(None)) => Ok(Async::Ready(None)),
                    Ok(Async::NotReady) => Ok(Async::NotReady),
                    Err(::Error::Io(err)) => Err(err),
                    Err(err) => Ok(Async::Ready(Some(Frame::Error {
                        error: err,
                    }))),
                };
            } else if self.can_write_continue() {
                try_nb!(self.flush());
            } else if self.can_read_body() {
                return self.read_body()
                    .map(|async| async.map(|chunk| Some(Frame::Body {
                        chunk: chunk
                    })))
                    .or_else(|err| {
                        self.state.close_read();
                        Ok(Async::Ready(Some(Frame::Error { error: err.into() })))
                    });
            } else {
                trace!("poll when on keep-alive");
                if !T::should_read_first() {
                    self.try_empty_read()?;
                    if self.is_read_closed() {
                        return Ok(Async::Ready(None));
                    }
                }
                self.maybe_park_read();
                return Ok(Async::NotReady);
            }
        }
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

    pub fn can_write_continue(&self) -> bool {
        match self.state.writing {
            Writing::Continue(..) => true,
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

    pub fn read_head(&mut self) -> Poll<Option<(super::MessageHead<T::Incoming>, bool)>, ::Error> {
        debug_assert!(self.can_read_head());
        trace!("Conn::read_head");

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
                let was_mid_parse = !self.io.read_buf().is_empty();
                return if was_mid_parse || must_error {
                    debug!("parse error ({}) with {} bytes", e, self.io.read_buf().len());
                    Err(e)
                } else {
                    debug!("read eof");
                    Ok(Async::Ready(None))
                };
            }
        };

        match version {
            HttpVersion::Http10 | HttpVersion::Http11 => {
                let decoder = match T::decoder(&head, &mut self.state.method) {
                    Ok(d) => d,
                    Err(e) => {
                        debug!("decoder error = {:?}", e);
                        self.state.close_read();
                        return Err(e);
                    }
                };

                debug!("incoming body is {}", decoder);

                self.state.busy();
                if head.expecting_continue() {
                    let msg = b"HTTP/1.1 100 Continue\r\n\r\n";
                    self.state.writing = Writing::Continue(Cursor::new(msg));
                }
                let wants_keep_alive = head.should_keep_alive();
                self.state.keep_alive &= wants_keep_alive;
                let (body, reading) = if decoder.is_eof() {
                    (false, Reading::KeepAlive)
                } else {
                    (true, Reading::Body(decoder))
                };
                self.state.reading = reading;
                if !body {
                    self.try_keep_alive();
                }
                Ok(Async::Ready(Some((head, body))))
            },
            _ => {
                error!("unimplemented HTTP Version = {:?}", version);
                self.state.close_read();
                Err(::Error::Version)
            }
        }
    }

    pub fn read_body(&mut self) -> Poll<Option<super::Chunk>, io::Error> {
        debug_assert!(self.can_read_body());

        trace!("Conn::read_body");

        let (reading, ret) = match self.state.reading {
            Reading::Body(ref mut decoder) => {
                let slice = try_ready!(decoder.decode(&mut self.io));
                if !slice.is_empty() {
                    return Ok(Async::Ready(Some(super::Chunk::from(slice))));
                } else if decoder.is_eof() {
                    debug!("incoming body completed");
                    (Reading::KeepAlive, Ok(Async::Ready(None)))
                } else {
                    trace!("decode stream unexpectedly ended");
                    //TODO: Should this return an UnexpectedEof?
                    (Reading::Closed, Ok(Async::Ready(None)))
                }

            },
            _ => unreachable!("read_body invalid state: {:?}", self.state.reading),
        };

        self.state.reading = reading;
        self.try_keep_alive();
        ret
    }

    pub fn maybe_park_read(&mut self) {
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
    pub fn try_empty_read(&mut self) -> io::Result<()> {
        assert!(!self.can_read_head() && !self.can_read_body());

        if !self.io.read_buf().is_empty() {
            debug!("received an unexpected {} bytes", self.io.read_buf().len());
            Err(io::Error::new(io::ErrorKind::InvalidData, "unexpected bytes after message ended"))
        } else {
             match self.io.read_from_io() {
                Ok(Async::Ready(0)) => {
                    trace!("try_empty_read; found EOF on connection: {:?}", self.state);
                    let must_error = self.should_error_on_eof();
                    // order is important: must_error needs state BEFORE close_read
                    self.state.close_read();
                    if must_error {
                        Err(io::Error::new(io::ErrorKind::UnexpectedEof, "unexpected EOF waiting for response"))
                    } else {
                        Ok(())
                    }
                },
                Ok(Async::Ready(n)) => {
                    debug!("received {} bytes on an idle connection", n);
                    Err(io::Error::new(io::ErrorKind::InvalidData, "unexpected bytes after message ended"))
                },
                Ok(Async::NotReady) => {
                    Ok(())
                },
                Err(e) => {
                    self.state.close();
                    Err(e)
                }
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
            Writing::Continue(..) |
            Writing::Body(..) |
            Writing::Ending(..) => return,
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
        match self.state.writing {
            Writing::Continue(..) | Writing::Init => true,
            _ => false
        }
    }

    pub fn can_write_body(&self) -> bool {
        match self.state.writing {
            Writing::Body(..) => true,
            Writing::Continue(..) |
            Writing::Init |
            Writing::Ending(..) |
            Writing::KeepAlive |
            Writing::Closed => false,
        }
    }

    pub fn has_queued_body(&self) -> bool {
        match self.state.writing {
            Writing::Body(_, Some(_)) => true,
            _ => false,
        }
    }

    pub fn write_head(&mut self, head: super::MessageHead<T::Outgoing>, body: bool) {
        debug_assert!(self.can_write_head());

        let wants_keep_alive = head.should_keep_alive();
        self.state.keep_alive &= wants_keep_alive;
        let buf = self.io.write_buf_mut();
        // if a 100-continue has started but not finished sending, tack the
        // remainder on to the start of the buffer.
        if let Writing::Continue(ref pending) = self.state.writing {
            if pending.has_started() {
                buf.extend_from_slice(pending.buf());
            }
        }
        let encoder = T::encode(head, body, &mut self.state.method, buf);
        self.state.writing = if !encoder.is_eof() {
            Writing::Body(encoder, None)
        } else {
            Writing::KeepAlive
        };
    }

    pub fn write_body(&mut self, chunk: Option<B>) -> StartSend<Option<B>, io::Error> {
        debug_assert!(self.can_write_body());

        if self.has_queued_body() {
            try!(self.flush());

            if !self.can_write_body() {
                if chunk.as_ref().map(|c| c.as_ref().len()).unwrap_or(0) == 0 {
                    return Ok(AsyncSink::NotReady(chunk));
                } else {
                    return Ok(AsyncSink::Ready);
                }
            }
        }

        let state = match self.state.writing {
            Writing::Body(ref mut encoder, ref mut queued) => {
                if queued.is_some() {
                    return Ok(AsyncSink::NotReady(chunk));
                }
                if let Some(chunk) = chunk {
                    if chunk.as_ref().is_empty() {
                        return Ok(AsyncSink::Ready);
                    }

                    let mut cursor = Cursor::new(chunk);
                    match encoder.encode(&mut self.io, cursor.buf()) {
                        Ok(n) => {
                            cursor.consume(n);

                            if !cursor.is_written() {
                                trace!("Conn::start_send frame not written, queued");
                                *queued = Some(cursor);
                            }
                        },
                        Err(e) => match e.kind() {
                            io::ErrorKind::WouldBlock => {
                                trace!("Conn::start_send frame not written, queued");
                                *queued = Some(cursor);
                            },
                            _ => return Err(e)
                        }
                    }

                    if encoder.is_eof() {
                        Writing::KeepAlive
                    } else {
                        return Ok(AsyncSink::Ready);
                    }
                } else {
                    // end of stream, that means we should try to eof
                    match encoder.eof() {
                        Ok(Some(end)) => Writing::Ending(Cursor::new(end)),
                        Ok(None) => Writing::KeepAlive,
                        Err(_not_eof) => Writing::Closed,
                    }
                }
            },
            _ => unreachable!("write_body invalid state: {:?}", self.state.writing),
        };

        self.state.writing = state;
        Ok(AsyncSink::Ready)
    }

    fn write_queued(&mut self) -> Poll<(), io::Error> {
        trace!("Conn::write_queued()");
        let state = match self.state.writing {
            Writing::Continue(ref mut queued) => {
                let n = self.io.buffer(queued.buf());
                queued.consume(n);
                if queued.is_written() {
                    Writing::Init
                } else {
                    return Ok(Async::NotReady);
                }
            }
            Writing::Body(ref mut encoder, ref mut queued) => {
                let complete = if let Some(chunk) = queued.as_mut() {
                    let n = try_nb!(encoder.encode(&mut self.io, chunk.buf()));
                    chunk.consume(n);
                    chunk.is_written()
                } else {
                    true
                };
                trace!("Conn::write_queued complete = {}", complete);
                return if complete {
                    *queued = None;
                    Ok(Async::Ready(()))
                } else {
                    Ok(Async::NotReady)
                };
            },
            Writing::Ending(ref mut ending) => {
                let n = self.io.buffer(ending.buf());
                ending.consume(n);
                if ending.is_written() {
                    Writing::KeepAlive
                } else {
                    return Ok(Async::NotReady);
                }
            },
            _ => return Ok(Async::Ready(())),
        };
        self.state.writing = state;
        Ok(Async::Ready(()))
    }

    pub fn flush(&mut self) -> Poll<(), io::Error> {
        loop {
            let queue_finished = try!(self.write_queued()).is_ready();
            try_nb!(self.io.flush());
            if queue_finished {
                break;
            }
        }
        self.try_keep_alive();
        trace!("flushed {:?}", self.state);
        Ok(Async::Ready(()))

    }

    pub fn shutdown(&mut self) -> Poll<(), io::Error> {
        match self.io.io_mut().shutdown() {
            Ok(Async::NotReady) => Ok(Async::NotReady),
            Ok(Async::Ready(())) => {
                trace!("shut down IO");
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
}

// ==== tokio_proto impl ====

#[cfg(feature = "tokio-proto")]
impl<I, B, T, K> Stream for Conn<I, B, T, K>
where I: AsyncRead + AsyncWrite,
      B: AsRef<[u8]>,
      T: Http1Transaction,
      K: KeepAlive,
      T::Outgoing: fmt::Debug {
    type Item = Frame<super::MessageHead<T::Incoming>, super::Chunk, ::Error>;
    type Error = io::Error;

    #[inline]
    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        self.poll_incoming().map_err(|err| {
            debug!("poll error: {}", err);
            err
        })
    }
}

#[cfg(feature = "tokio-proto")]
impl<I, B, T, K> Sink for Conn<I, B, T, K>
where I: AsyncRead + AsyncWrite,
      B: AsRef<[u8]>,
      T: Http1Transaction,
      K: KeepAlive,
      T::Outgoing: fmt::Debug {
    type SinkItem = Frame<super::MessageHead<T::Outgoing>, B, ::Error>;
    type SinkError = io::Error;

    #[inline]
    fn start_send(&mut self, frame: Self::SinkItem) -> StartSend<Self::SinkItem, Self::SinkError> {
        trace!("Conn::start_send( frame={:?} )", DebugFrame(&frame));

        let frame: Self::SinkItem = match frame {
            Frame::Message { message: head, body } => {
                if self.can_write_head() {
                    self.write_head(head, body);
                    return Ok(AsyncSink::Ready);
                } else {
                    Frame::Message { message: head, body: body }
                }
            },
            Frame::Body { chunk } => {
                if self.can_write_body() {
                    return self.write_body(chunk)
                        .map(|async| {
                            match async {
                                AsyncSink::Ready => AsyncSink::Ready,
                                AsyncSink::NotReady(chunk) => AsyncSink::NotReady(Frame::Body {
                                    chunk: chunk,
                                })
                            }
                        });
                // This allows when chunk is `None`, or `Some([])`.
                } else if chunk.as_ref().map(|c| c.as_ref().len()).unwrap_or(0) == 0 {
                    return Ok(AsyncSink::Ready);
                } else {
                    Frame::Body { chunk: chunk }
                }
            },
            Frame::Error { error } => {
                debug!("received error, closing: {:?}", error);
                self.state.close();
                return Ok(AsyncSink::Ready);
            },
        };

        error!("writing illegal frame; state={:?}, frame={:?}", self.state.writing, DebugFrame(&frame));
        Err(io::Error::new(io::ErrorKind::InvalidInput, "illegal frame"))

    }

    #[inline]
    fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
        trace!("Conn::poll_complete()");
        self.flush().map_err(|err| {
            debug!("error writing: {}", err);
            err
        })
    }

    #[inline]
    fn close(&mut self) -> Poll<(), Self::SinkError> {
        try_ready!(self.flush());
        self.shutdown()
    }
}

#[cfg(feature = "tokio-proto")]
impl<I, B, T, K> Transport for Conn<I, B, T, K>
where I: AsyncRead + AsyncWrite + 'static,
      B: AsRef<[u8]> + 'static,
      T: Http1Transaction + 'static,
      K: KeepAlive + 'static,
      T::Outgoing: fmt::Debug {}

impl<I, B: AsRef<[u8]>, T, K: KeepAlive> fmt::Debug for Conn<I, B, T, K> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Conn")
            .field("state", &self.state)
            .field("io", &self.io)
            .finish()
    }
}

struct State<B, K> {
    keep_alive: K,
    method: Option<Method>,
    read_task: Option<Task>,
    reading: Reading,
    writing: Writing<B>,
}

#[derive(Debug)]
enum Reading {
    Init,
    Body(Decoder),
    KeepAlive,
    Closed,
}

enum Writing<B> {
    Continue(Cursor<&'static [u8]>),
    Init,
    Body(Encoder, Option<Cursor<B>>),
    Ending(Cursor<&'static [u8]>),
    KeepAlive,
    Closed,
}

impl<B: AsRef<[u8]>, K: KeepAlive> fmt::Debug for State<B, K> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("State")
            .field("reading", &self.reading)
            .field("writing", &self.writing)
            .field("keep_alive", &self.keep_alive.status())
            //.field("method", &self.method)
            .field("read_task", &self.read_task)
            .finish()
    }
}

impl<B: AsRef<[u8]>> fmt::Debug for Writing<B> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Writing::Continue(ref buf) => f.debug_tuple("Continue")
                .field(buf)
                .finish(),
            Writing::Init => f.write_str("Init"),
            Writing::Body(ref enc, ref queued) => f.debug_tuple("Body")
                .field(enc)
                .field(queued)
                .finish(),
            Writing::Ending(ref ending) => f.debug_tuple("Ending")
                .field(ending)
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

pub trait KeepAlive: fmt::Debug + ::std::ops::BitAndAssign<bool> {
    fn busy(&mut self);
    fn disable(&mut self);
    fn idle(&mut self);
    fn status(&self) -> KA;
}

#[derive(Clone, Copy, Debug)]
pub enum KA {
    Idle,
    Busy,
    Disabled,
}

impl Default for KA {
    fn default() -> KA {
        KA::Busy
    }
}

impl KeepAlive for KA {
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

impl<B, K: KeepAlive> State<B, K> {
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

// The DebugFrame and DebugChunk are simple Debug implementations that allow
// us to dump the frame into logs, without logging the entirety of the bytes.
#[cfg(feature = "tokio-proto")]
struct DebugFrame<'a, T: fmt::Debug + 'a, B: AsRef<[u8]> + 'a>(&'a Frame<super::MessageHead<T>, B, ::Error>);

#[cfg(feature = "tokio-proto")]
impl<'a, T: fmt::Debug + 'a, B: AsRef<[u8]> + 'a> fmt::Debug for DebugFrame<'a, T, B> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self.0 {
            Frame::Message { ref body, .. } => {
                f.debug_struct("Message")
                    .field("body", body)
                    .finish()
            },
            Frame::Body { chunk: Some(ref chunk) } => {
                f.debug_struct("Body")
                    .field("bytes", &chunk.as_ref().len())
                    .finish()
            },
            Frame::Body { chunk: None } => {
                f.debug_struct("Body")
                    .field("bytes", &None::<()>)
                    .finish()
            },
            Frame::Error { ref error } => {
                f.debug_struct("Error")
                    .field("error", error)
                    .finish()
            }
        }
    }
}

#[cfg(test)]
#[cfg(feature = "tokio-proto")]
//TODO: rewrite these using dispatch instead of tokio-proto API
mod tests {
    use futures::{Async, Future, Stream, Sink};
    use futures::future;
    use tokio_proto::streaming::pipeline::Frame;

    use proto::{self, ClientTransaction, MessageHead, ServerTransaction};
    use super::super::h1::Encoder;
    use mock::AsyncIo;

    use super::{Conn, Decoder, Reading, Writing};
    use ::uri::Uri;

    use std::str::FromStr;

    impl<T> Writing<T> {
        fn is_queued(&self) -> bool {
            match *self {
                Writing::Body(_, Some(_)) => true,
                _ => false,
            }
        }
    }

    #[test]
    fn test_conn_init_read() {
        let good_message = b"GET / HTTP/1.1\r\n\r\n".to_vec();
        let len = good_message.len();
        let io = AsyncIo::new_buf(good_message, len);
        let mut conn = Conn::<_, proto::Chunk, ServerTransaction>::new(io, Default::default());

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
            let mut conn = Conn::<_, proto::Chunk, ServerTransaction>::new(io, Default::default());
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
        let mut conn = Conn::<_, proto::Chunk, ServerTransaction>::new(io, Default::default());
        conn.state.idle();

        match conn.poll().unwrap() {
            Async::Ready(None) => {},
            other => panic!("frame is not None: {:?}", other)
        }
    }

    #[test]
    fn test_conn_init_read_eof_idle_partial_parse() {
        let io = AsyncIo::new_buf(b"GET / HTTP/1.1".to_vec(), 100);
        let mut conn = Conn::<_, proto::Chunk, ServerTransaction>::new(io, Default::default());
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
            let mut conn = Conn::<_, proto::Chunk, ServerTransaction>::new(io, Default::default());
            conn.state.busy();

            match conn.poll().unwrap() {
                Async::Ready(None) => {},
                other => panic!("unexpected frame: {:?}", other)
            }

            // client
            let io = AsyncIo::new_eof();
            let mut conn = Conn::<_, proto::Chunk, ClientTransaction>::new(io, Default::default());
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
            let mut conn = Conn::<_, proto::Chunk, ClientTransaction>::new(io, Default::default());
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
            let mut conn = Conn::<_, proto::Chunk, ClientTransaction>::new(io, Default::default());
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
    fn test_conn_closed_read() {
        let io = AsyncIo::new_buf(vec![], 0);
        let mut conn = Conn::<_, proto::Chunk, ServerTransaction>::new(io, Default::default());
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
            let mut conn = Conn::<_, proto::Chunk, ServerTransaction>::new(io, Default::default());
            let max = ::proto::io::MAX_BUFFER_SIZE + 4096;
            conn.state.writing = Writing::Body(Encoder::length((max * 2) as u64), None);

            assert!(conn.start_send(Frame::Body { chunk: Some(vec![b'a'; 1024 * 8].into()) }).unwrap().is_ready());
            assert!(!conn.state.writing.is_queued());

            assert!(conn.start_send(Frame::Body { chunk: Some(vec![b'b'; max].into()) }).unwrap().is_ready());
            assert!(conn.state.writing.is_queued());

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
            let mut conn = Conn::<_, proto::Chunk, ServerTransaction>::new(io, Default::default());
            conn.state.writing = Writing::Body(Encoder::chunked(), None);

            assert!(conn.start_send(Frame::Body { chunk: Some("headers".into()) }).unwrap().is_ready());
            assert!(conn.start_send(Frame::Body { chunk: Some(vec![b'x'; 8192].into()) }).unwrap().is_ready());
            Ok(())
        }).wait();
    }

    #[test]
    fn test_conn_body_flush() {
        let _: Result<(), ()> = future::lazy(|| {
            let io = AsyncIo::new_buf(vec![], 1024 * 1024 * 5);
            let mut conn = Conn::<_, proto::Chunk, ServerTransaction>::new(io, Default::default());
            conn.state.writing = Writing::Body(Encoder::length(1024 * 1024), None);
            assert!(conn.start_send(Frame::Body { chunk: Some(vec![b'a'; 1024 * 1024].into()) }).unwrap().is_ready());
            assert!(conn.state.writing.is_queued());
            assert!(conn.poll_complete().unwrap().is_ready());
            assert!(!conn.state.writing.is_queued());
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
            let mut conn = Conn::<_, proto::Chunk, ServerTransaction>::new(io, Default::default());
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
            let mut conn = Conn::<_, proto::Chunk, ServerTransaction>::new(io, Default::default());
            conn.state.writing = Writing::KeepAlive;
            assert!(conn.poll_complete().unwrap().is_ready());
            Ok::<(), ()>(())
        });
        ::futures::executor::spawn(f).poll_future_notify(&car(false), 0).unwrap();


        // test that flushing and writing isn't done doesn't unpark
        let f = future::lazy(|| {
            let io = AsyncIo::new_buf(vec![], 4096);
            let mut conn = Conn::<_, proto::Chunk, ServerTransaction>::new(io, Default::default());
            conn.state.reading = Reading::KeepAlive;
            assert!(conn.poll().unwrap().is_not_ready());
            conn.state.writing = Writing::Body(Encoder::length(5_000), None);
            assert!(conn.poll_complete().unwrap().is_ready());
            Ok::<(), ()>(())
        });
        ::futures::executor::spawn(f).poll_future_notify(&car(false), 0).unwrap();
    }

    #[test]
    fn test_conn_closed_write() {
        let io = AsyncIo::new_buf(vec![], 0);
        let mut conn = Conn::<_, proto::Chunk, ServerTransaction>::new(io, Default::default());
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
        let mut conn = Conn::<_, proto::Chunk, ServerTransaction>::new(io, Default::default());
        conn.state.writing = Writing::KeepAlive;

        assert!(conn.start_send(Frame::Body { chunk: None }).unwrap().is_ready());
        assert!(conn.start_send(Frame::Body { chunk: Some(Vec::new().into()) }).unwrap().is_ready());
        conn.start_send(Frame::Body { chunk: Some(vec![b'a'].into()) }).unwrap_err();
    }
}
