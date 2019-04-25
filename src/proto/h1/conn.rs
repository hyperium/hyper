use std::fmt;
use std::io::{self};
use std::marker::PhantomData;

use bytes::{Buf, Bytes};
use futures::{Async, Poll};
use http::{HeaderMap, Method, Version};
use http::header::{HeaderValue, CONNECTION};
use tokio_io::{AsyncRead, AsyncWrite};

use ::Chunk;
use proto::{BodyLength, DecodedLength, MessageHead};
use headers::connection_keep_alive;
use super::io::{Buffered};
use super::{EncodedBuf, Encode, Encoder, /*Decode,*/ Decoder, Http1Transaction, ParseContext};

const H2_PREFACE: &'static [u8] = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";

/// This handles a connection, which will have been established over an
/// `AsyncRead + AsyncWrite` (like a socket), and will likely include multiple
/// `Transaction`s over HTTP.
///
/// The connection will determine when a message begins and ends as well as
/// determine if this connection can be kept alive after the message,
/// or if it is complete.
pub(crate) struct Conn<I, B, T> {
    io: Buffered<I, EncodedBuf<B>>,
    state: State,
    _marker: PhantomData<T>
}

impl<I, B, T> Conn<I, B, T>
where I: AsyncRead + AsyncWrite,
      B: Buf,
      T: Http1Transaction,
{
    pub fn new(io: I) -> Conn<I, B, T> {
        Conn {
            io: Buffered::new(io),
            state: State {
                allow_half_close: true,
                cached_headers: None,
                error: None,
                keep_alive: KA::Busy,
                method: None,
                title_case_headers: false,
                notify_read: false,
                reading: Reading::Init,
                writing: Writing::Init,
                upgrade: None,
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

    pub fn set_read_buf_exact_size(&mut self, sz: usize) {
        self.io.set_read_buf_exact_size(sz);
    }

    pub fn set_write_strategy_flatten(&mut self) {
        self.io.set_write_strategy_flatten();
    }

    pub fn set_title_case_headers(&mut self) {
        self.state.title_case_headers = true;
    }

    pub(crate) fn set_disable_half_close(&mut self) {
        self.state.allow_half_close = false;
    }

    pub fn into_inner(self) -> (I, Bytes) {
        self.io.into_inner()
    }

    pub fn pending_upgrade(&mut self) -> Option<::upgrade::Pending> {
        self.state.upgrade.take()
    }

    pub fn is_read_closed(&self) -> bool {
        self.state.is_read_closed()
    }

    pub fn is_write_closed(&self) -> bool {
        self.state.is_write_closed()
    }

    pub fn can_read_head(&self) -> bool {
        match self.state.reading {
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

    fn has_h2_prefix(&self) -> bool {
        let read_buf = self.io.read_buf();
        read_buf.len() >= 24 && read_buf[..24] == *H2_PREFACE
    }

    pub fn read_head(&mut self) -> Poll<Option<(MessageHead<T::Incoming>, DecodedLength, bool)>, ::Error> {
        debug_assert!(self.can_read_head());
        trace!("Conn::read_head");

        let msg = match self.io.parse::<T>(ParseContext {
            cached_headers: &mut self.state.cached_headers,
            req_method: &mut self.state.method,
        }) {
            Ok(Async::Ready(msg)) => msg,
            Ok(Async::NotReady) => return Ok(Async::NotReady),
            Err(e) => return self.on_read_head_error(e),
        };

        // Note: don't deconstruct `msg` into local variables, it appears
        // the optimizer doesn't remove the extra copies.

        debug!("incoming body is {}", msg.decode);

        self.state.busy();
        self.state.keep_alive &= msg.keep_alive;
        self.state.version = msg.head.version;

        if msg.decode == DecodedLength::ZERO {
            debug_assert!(!msg.expect_continue, "expect-continue needs a body");
            self.state.reading = Reading::KeepAlive;
            if !T::should_read_first() {
                self.try_keep_alive();
            }
        } else {
            if msg.expect_continue {
                let cont = b"HTTP/1.1 100 Continue\r\n\r\n";
                self.io.headers_buf().extend_from_slice(cont);
            }
            self.state.reading = Reading::Body(Decoder::new(msg.decode));
        };

        Ok(Async::Ready(Some((msg.head, msg.decode, msg.wants_upgrade))))
    }

    fn on_read_head_error<Z>(&mut self, e: ::Error) -> Poll<Option<Z>, ::Error> {
        // If we are currently waiting on a message, then an empty
        // message should be reported as an error. If not, it is just
        // the connection closing gracefully.
        let must_error = self.should_error_on_eof();
        self.state.close_read();
        self.io.consume_leading_lines();
        let was_mid_parse = e.is_parse() || !self.io.read_buf().is_empty();
        if was_mid_parse || must_error {
            // We check if the buf contains the h2 Preface
            debug!("parse error ({}) with {} bytes", e, self.io.read_buf().len());
            self.on_parse_error(e)
                .map(|()| Async::NotReady)
        } else {
            debug!("read eof");
            Ok(Async::Ready(None))
        }
    }

    pub fn read_body(&mut self) -> Poll<Option<Chunk>, io::Error> {
        debug_assert!(self.can_read_body());

        let (reading, ret) = match self.state.reading {
            Reading::Body(ref mut decoder) => {
                match decoder.decode(&mut self.io) {
                    Ok(Async::Ready(slice)) => {
                        let (reading, chunk) = if decoder.is_eof() {
                            debug!("incoming body completed");
                            (Reading::KeepAlive, if !slice.is_empty() {
                                Some(Chunk::from(slice))
                            } else {
                                None
                            })
                        } else if slice.is_empty() {
                            error!("decode stream unexpectedly ended");
                            // This should be unreachable, since all 3 decoders
                            // either set eof=true or return an Err when reading
                            // an empty slice...
                            (Reading::Closed, None)
                        } else {
                            return Ok(Async::Ready(Some(Chunk::from(slice))));
                        };
                        (reading, Ok(Async::Ready(chunk)))
                    },
                    Ok(Async::NotReady) => return Ok(Async::NotReady),
                    Err(e) => {
                        debug!("decode stream error: {}", e);
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

    pub fn wants_read_again(&mut self) -> bool {
        let ret = self.state.notify_read;
        self.state.notify_read = false;
        ret
    }

    pub fn read_keep_alive(&mut self) -> Poll<(), ::Error> {
        debug_assert!(!self.can_read_head() && !self.can_read_body());

        if self.is_mid_message() {
            self.mid_message_detect_eof()
        } else {
            self.require_empty_read()
        }
    }

    fn is_mid_message(&self) -> bool {
        match (&self.state.reading, &self.state.writing) {
            (&Reading::Init, &Writing::Init) => false,
            _ => true,
        }
    }

    // This will check to make sure the io object read is empty.
    //
    // This should only be called for Clients wanting to enter the idle
    // state.
    fn require_empty_read(&mut self) -> Poll<(), ::Error> {
        debug_assert!(!self.can_read_head() && !self.can_read_body());
        debug_assert!(!self.is_mid_message());
        debug_assert!(T::is_client());

        if !self.io.read_buf().is_empty() {
            debug!("received an unexpected {} bytes", self.io.read_buf().len());
            return Err(::Error::new_unexpected_message());
        }

        let num_read = try_ready!(self.force_io_read().map_err(::Error::new_io));

        if num_read == 0 {
            let ret = if self.should_error_on_eof() {
                trace!("found unexpected EOF on busy connection: {:?}", self.state);
                Err(::Error::new_incomplete())
            } else {
                trace!("found EOF on idle connection, closing");
                Ok(Async::Ready(()))
            };

            // order is important: should_error needs state BEFORE close_read
            self.state.close_read();
            return ret;
        }

        debug!("received unexpected {} bytes on an idle connection", num_read);
        Err(::Error::new_unexpected_message())
    }

    fn mid_message_detect_eof(&mut self) -> Poll<(), ::Error> {
        debug_assert!(!self.can_read_head() && !self.can_read_body());
        debug_assert!(self.is_mid_message());

        if self.state.allow_half_close || !self.io.read_buf().is_empty() {
            return Ok(Async::NotReady);
        }

        let num_read = try_ready!(self.force_io_read().map_err(::Error::new_io));

        if num_read == 0 {
            trace!("found unexpected EOF on busy connection: {:?}", self.state);
            self.state.close_read();
            Err(::Error::new_incomplete())
        } else {
            Ok(Async::Ready(()))
        }
    }

    fn force_io_read(&mut self) -> Poll<usize, io::Error> {
         self.io.read_from_io().map_err(|e| {
            trace!("force_io_read; io error = {:?}", e);
            self.state.close();
            e
         })
    }


    fn maybe_notify(&mut self) {
        // its possible that we returned NotReady from poll() without having
        // exhausted the underlying Io. We would have done this when we
        // determined we couldn't keep reading until we knew how writing
        // would finish.



        match self.state.reading {
            Reading::Body(..) |
            Reading::KeepAlive |
            Reading::Closed => return,
            Reading::Init => (),
        };

        match self.state.writing {
            Writing::Body(..) => return,
            Writing::Init |
            Writing::KeepAlive |
            Writing::Closed => (),
        }

        if !self.io.is_read_blocked() {
            if self.io.read_buf().is_empty() {
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
            self.state.notify_read = true;
        }
    }

    fn try_keep_alive(&mut self) {
        self.state.try_keep_alive::<T>();
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

    pub fn write_head(&mut self, head: MessageHead<T::Outgoing>, body: Option<BodyLength>) {
        if let Some(encoder) = self.encode_head(head, body) {
            self.state.writing = if !encoder.is_eof() {
                Writing::Body(encoder)
            } else if encoder.is_last() {
                Writing::Closed
            } else {
                Writing::KeepAlive
            };
        }
    }

    pub fn write_full_msg(&mut self, head: MessageHead<T::Outgoing>, body: B) {
        if let Some(encoder) = self.encode_head(head, Some(BodyLength::Known(body.remaining() as u64))) {
            let is_last = encoder.is_last();
            // Make sure we don't write a body if we weren't actually allowed
            // to do so, like because its a HEAD request.
            if !encoder.is_eof() {
                encoder.danger_full_buf(body, self.io.write_buf());
            }
            self.state.writing = if is_last {
                Writing::Closed
            } else {
                Writing::KeepAlive
            }
        }
    }

    fn encode_head(&mut self, mut head: MessageHead<T::Outgoing>, body: Option<BodyLength>) -> Option<Encoder> {
        debug_assert!(self.can_write_head());

        if !T::should_read_first() {
            self.state.busy();
        }

        self.enforce_version(&mut head);

        let buf = self.io.headers_buf();
        match T::encode(Encode {
            head: &mut head,
            body,
            keep_alive: self.state.wants_keep_alive(),
            req_method: &mut self.state.method,
            title_case_headers: self.state.title_case_headers,
        }, buf) {
            Ok(encoder) => {
                debug_assert!(self.state.cached_headers.is_none());
                debug_assert!(head.headers.is_empty());
                self.state.cached_headers = Some(head.headers);
                Some(encoder)
            },
            Err(err) => {
                self.state.error = Some(err);
                self.state.writing = Writing::Closed;
                None
            },
        }
    }

    // Fix keep-alives when Connection: keep-alive header is not present
    fn fix_keep_alive(&mut self, head: &mut MessageHead<T::Outgoing>) {
        let outgoing_is_keep_alive = head
            .headers
            .get(CONNECTION)
            .and_then(|value| Some(connection_keep_alive(value)))
            .unwrap_or(false);

        if !outgoing_is_keep_alive {
            match head.version {
                // If response is version 1.0 and keep-alive is not present in the response,
                // disable keep-alive so the server closes the connection
                Version::HTTP_10 => self.state.disable_keep_alive(),
                // If response is version 1.1 and keep-alive is wanted, add
                // Connection: keep-alive header when not present
                Version::HTTP_11 => if self.state.wants_keep_alive() {
                    head.headers
                        .insert(CONNECTION, HeaderValue::from_static("keep-alive"));
                },
                _ => (),
            }
        }
    }

    // If we know the remote speaks an older version, we try to fix up any messages
    // to work with our older peer.
    fn enforce_version(&mut self, head: &mut MessageHead<T::Outgoing>) {

        match self.state.version {
            Version::HTTP_10 => {
                // Fixes response or connection when keep-alive header is not present
                self.fix_keep_alive(head);
                // If the remote only knows HTTP/1.0, we should force ourselves
                // to do only speak HTTP/1.0 as well.
                head.version = Version::HTTP_10;
            },
            _ => {
                // If the remote speaks HTTP/1.1, then it *should* be fine with
                // both HTTP/1.0 and HTTP/1.1 from us. So again, we just let
                // the user's headers be.
            }
        }
    }

    pub fn write_body(&mut self, chunk: B) {
        debug_assert!(self.can_write_body() && self.can_buffer_body());
        // empty chunks should be discarded at Dispatcher level
        debug_assert!(chunk.remaining() != 0);

        let state = match self.state.writing {
            Writing::Body(ref mut encoder) => {
                self.io.buffer(encoder.encode(chunk));

                if encoder.is_eof() {
                    if encoder.is_last() {
                        Writing::Closed
                    } else {
                        Writing::KeepAlive
                    }
                } else {
                    return;
                }
            },
            _ => unreachable!("write_body invalid state: {:?}", self.state.writing),
        };

        self.state.writing = state;
    }

    pub fn write_body_and_end(&mut self, chunk: B) {
        debug_assert!(self.can_write_body() && self.can_buffer_body());
        // empty chunks should be discarded at Dispatcher level
        debug_assert!(chunk.remaining() != 0);

        let state = match self.state.writing {
            Writing::Body(ref encoder) => {
                let can_keep_alive = encoder.encode_and_end(chunk, self.io.write_buf());
                if can_keep_alive {
                    Writing::KeepAlive
                } else {
                    Writing::Closed
                }
            },
            _ => unreachable!("write_body invalid state: {:?}", self.state.writing),
        };

        self.state.writing = state;
    }

    pub fn end_body(&mut self) {
        debug_assert!(self.can_write_body());

        let state = match self.state.writing {
            Writing::Body(ref mut encoder) => {
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
            },
            _ => return,
        };

        self.state.writing = state;

    }

    // When we get a parse error, depending on what side we are, we might be able
    // to write a response before closing the connection.
    //
    // - Client: there is nothing we can do
    // - Server: if Response hasn't been written yet, we can send a 4xx response
    fn on_parse_error(&mut self, err: ::Error) -> ::Result<()> {

        match self.state.writing {
            Writing::Init => {
                if self.has_h2_prefix() {
                    return Err(::Error::new_version_h2())
                }
                if let Some(msg) = T::on_error(&err) {
                    // Drop the cached headers so as to not trigger a debug
                    // assert in `write_head`...
                    self.state.cached_headers.take();
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
        trace!("flushed({}): {:?}", T::LOG, self.state);
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

    pub(super) fn on_upgrade(&mut self) -> ::upgrade::OnUpgrade {
        trace!("{}: prepare possible HTTP upgrade", T::LOG);
        self.state.prepare_upgrade()
    }

    // Used in h1::dispatch tests
    #[cfg(test)]
    pub(super) fn io_mut(&mut self) -> &mut I {
        self.io.io_mut()
    }
}

impl<I, B: Buf, T> fmt::Debug for Conn<I, B, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Conn")
            .field("state", &self.state)
            .field("io", &self.io)
            .finish()
    }
}

struct State {
    allow_half_close: bool,
    /// Re-usable HeaderMap to reduce allocating new ones.
    cached_headers: Option<HeaderMap>,
    /// If an error occurs when there wasn't a direct way to return it
    /// back to the user, this is set.
    error: Option<::Error>,
    /// Current keep-alive status.
    keep_alive: KA,
    /// If mid-message, the HTTP Method that started it.
    ///
    /// This is used to know things such as if the message can include
    /// a body or not.
    method: Option<Method>,
    title_case_headers: bool,
    /// Set to true when the Dispatcher should poll read operations
    /// again. See the `maybe_notify` method for more.
    notify_read: bool,
    /// State of allowed reads
    reading: Reading,
    /// State of allowed writes
    writing: Writing,
    /// An expected pending HTTP upgrade.
    upgrade: Option<::upgrade::Pending>,
    /// Either HTTP/1.0 or 1.1 connection
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
        let mut builder = f.debug_struct("State");
        builder
            .field("reading", &self.reading)
            .field("writing", &self.writing)
            .field("keep_alive", &self.keep_alive);

        // Only show error field if it's interesting...
        if let Some(ref error) = self.error {
            builder.field("error", error);
        }

        // Purposefully leaving off other fields..

        builder.finish()
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
            trace!("remote disabling keep-alive");
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

    fn try_keep_alive<T: Http1Transaction>(&mut self) {
        match (&self.reading, &self.writing) {
            (&Reading::KeepAlive, &Writing::KeepAlive) => {
                if let KA::Busy = self.keep_alive.status() {
                    self.idle::<T>();
                } else {
                    trace!("try_keep_alive({}): could keep-alive, but status = {:?}", T::LOG, self.keep_alive);
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

    fn idle<T: Http1Transaction>(&mut self) {
        debug_assert!(!self.is_idle(), "State::idle() called while idle");

        self.method = None;
        self.keep_alive.idle();
        if self.is_idle() {
            self.reading = Reading::Init;
            self.writing = Writing::Init;

            // !T::should_read_first() means Client.
            //
            // If Client connection has just gone idle, the Dispatcher
            // should try the poll loop one more time, so as to poll the
            // pending requests stream.
            if !T::should_read_first() {
                self.notify_read = true;
            }
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

    fn prepare_upgrade(&mut self) -> ::upgrade::OnUpgrade {
        debug_assert!(self.upgrade.is_none());
        let (tx, rx) = ::upgrade::pending();
        self.upgrade = Some(tx);
        rx
    }
}

#[cfg(test)]
//TODO: rewrite these using dispatch
mod tests {

    #[cfg(feature = "nightly")]
    #[bench]
    fn bench_read_head_short(b: &mut ::test::Bencher) {
        use super::*;
        let s = b"GET / HTTP/1.1\r\nHost: localhost:8080\r\n\r\n";
        let len = s.len();
        b.bytes = len as u64;

        let mut io = ::mock::AsyncIo::new_buf(Vec::new(), 0);
        io.panic();
        let mut conn = Conn::<_, ::Chunk, ::proto::h1::ServerTransaction>::new(io);
        *conn.io.read_buf_mut() = ::bytes::BytesMut::from(&s[..]);
        conn.state.cached_headers = Some(HeaderMap::with_capacity(2));

        b.iter(|| {
            match conn.read_head().unwrap() {
                Async::Ready(Some(x)) => {
                    ::test::black_box(&x);
                    let mut headers = x.0.headers;
                    headers.clear();
                    conn.state.cached_headers = Some(headers);
                },
                f => panic!("expected Ready(Some(..)): {:?}", f)
            }


            conn.io.read_buf_mut().reserve(1);
            unsafe {
                conn.io.read_buf_mut().set_len(len);
            }
            conn.state.reading = Reading::Init;
        });
    }
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
