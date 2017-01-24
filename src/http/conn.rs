use std::fmt;
use std::io::{self, Write};
use std::marker::PhantomData;
use std::time::Instant;

use futures::{Poll, Async, AsyncSink, Stream, Sink, StartSend};
use tokio::io::Io;
use tokio_proto::streaming::pipeline::{Frame, Transport};

use header::{ContentLength, TransferEncoding};
use http::{self, Http1Transaction};
use http::io::{Cursor, Buffered};
use http::h1::{Encoder, Decoder};
use version::HttpVersion;


/// This handles a connection, which will have been established over an
/// `Io` (like a socket), and will likely include multiple
/// `Transaction`s over HTTP.
///
/// The connection will determine when a message begins and ends as well as
/// determine if this  connection can be kept alive after the message,
/// or if it is complete.
pub struct Conn<I, T, K = KA> {
    io: Buffered<I>,
    state: State<K>,
    _marker: PhantomData<T>
}

impl<I: Io, T: Http1Transaction, K: KeepAlive> Conn<I, T, K> {
    pub fn new(io: I, keep_alive: K) -> Conn<I, T, K> {
        Conn {
            io: Buffered::new(io),
            state: State {
                reading: Reading::Init,
                writing: Writing::Init,
                keep_alive: keep_alive,
            },
            _marker: PhantomData,
        }
    }

    fn parse(&mut self) -> ::Result<Option<http::MessageHead<T::Incoming>>> {
        self.io.parse::<T>()
    }

    fn is_read_ready(&mut self) -> bool {
        match self.state.reading {
            Reading::Init |
            Reading::Body(..) => self.io.poll_read().is_ready(),
            Reading::KeepAlive | Reading::Closed => true,
        }
    }

    fn is_read_closed(&self) -> bool {
        self.state.is_read_closed()
    }

    #[allow(unused)]
    fn is_write_closed(&self) -> bool {
        self.state.is_write_closed()
    }

    fn can_read_head(&self) -> bool {
        match self.state.reading {
            Reading::Init => true,
            _ => false,
        }
    }

    fn can_read_body(&self) -> bool {
        match self.state.reading {
            Reading::Body(..) => true,
            _ => false,
        }
    }

    fn read_head(&mut self) -> Poll<Option<Frame<http::MessageHead<T::Incoming>, http::Chunk, ::Error>>, io::Error> {
        debug_assert!(self.can_read_head());
        trace!("Conn::read_head");

        let (version, head) = match self.parse() {
            Ok(Some(head)) => (head.version, head),
            Ok(None) => return Ok(Async::NotReady),
            Err(e) => {
                let must_respond_with_error = !self.state.was_idle();
                self.state.close();
                self.io.consume_leading_lines();
                let ret = if !self.io.read_buf().is_empty() {
                    error!("parse error ({}) with bytes: {:?}", e, self.io.read_buf());
                    Ok(Async::Ready(Some(Frame::Error { error: e })))
                } else {
                    trace!("parse error with 0 input, err = {:?}", e);
                    if must_respond_with_error {
                        match e {
                            ::Error::Io(io) => Err(io),
                            other => Err(io::Error::new(io::ErrorKind::UnexpectedEof, other)),
                        }
                    } else {
                        debug!("socket complete");
                        Ok(Async::Ready(None))
                    }
                };
                return ret;
            }
        };

        match version {
            HttpVersion::Http10 | HttpVersion::Http11 => {
                let decoder = match T::decoder(&head) {
                    Ok(d) => d,
                    Err(e) => {
                        error!("decoder error = {:?}", e);
                        self.state.close();
                        return Ok(Async::Ready(Some(Frame::Error { error: e })));
                    }
                };
                self.state.busy();
                let wants_keep_alive = head.should_keep_alive();
                self.state.keep_alive &= wants_keep_alive;
                let (body, reading) = if decoder.is_eof() {
                    (false, Reading::KeepAlive)
                } else {
                    (true, Reading::Body(decoder))
                };
                self.state.reading = reading;
                return Ok(Async::Ready(Some(Frame::Message { message: head, body: body })));
            },
            _ => {
                error!("unimplemented HTTP Version = {:?}", version);
                self.state.close();
                return Ok(Async::Ready(Some(Frame::Error { error: ::Error::Version })));
            }
        }
    }

    fn read_body(&mut self) -> Poll<Option<http::Chunk>, io::Error> {
        debug_assert!(self.can_read_body());

        trace!("Conn::read_body");

        let (reading, ret) = match self.state.reading {
            Reading::Body(ref mut decoder) => {
                let slice = try_nb!(decoder.decode(&mut self.io));
                if !slice.is_empty() {
                    return Ok(Async::Ready(Some(http::Chunk::from(slice))));
                } else {
                    if decoder.is_eof() {
                        (Reading::KeepAlive, Ok(Async::Ready(None)))
                    } else {
                        (Reading::Closed, Ok(Async::Ready(None)))
                    }
                }

            },
            Reading::Init | Reading::KeepAlive | Reading::Closed => unreachable!()
        };
        self.state.reading = reading;
        ret
    }

    fn can_write_head(&self) -> bool {
        match self.state.writing {
            Writing::Init => true,
            _ => false
        }
    }

    fn can_write_body(&self) -> bool {
        match self.state.writing {
            Writing::Body(..) => true,
            _ => false
        }
    }

    fn write_head(&mut self, mut head: http::MessageHead<T::Outgoing>, body: bool) -> StartSend<http::MessageHead<T::Outgoing>,io::Error> {
        debug_assert!(self.can_write_head());
        if !body {
            head.headers.remove::<TransferEncoding>();
            //TODO: check that this isn't a response to a HEAD
            //request, which could include the content-length
            //even if no body is to be written
            if T::should_set_length(&head) {
                head.headers.set(ContentLength(0));
            }
        }

        let wants_keep_alive = head.should_keep_alive();
        self.state.keep_alive &= wants_keep_alive;
        let mut buf = Vec::new();
        let encoder = T::encode(&mut head, &mut buf);
        self.io.buffer(buf);
        self.state.writing = if body {
            Writing::Body(encoder, None)
        } else {
            Writing::KeepAlive
        };

        Ok(AsyncSink::Ready)
    }

    fn write_body(&mut self, chunk: Option<http::Chunk>) -> StartSend<Option<http::Chunk>, io::Error> {
        debug_assert!(self.can_write_body());

        let state = match self.state.writing {
            Writing::Body(ref mut encoder, ref mut queued) => {
                if queued.is_some() {
                    return Ok(AsyncSink::NotReady(chunk));
                }
                let mut is_done = true;
                let mut wbuf = Cursor::new(match chunk {
                    Some(chunk) => {
                        is_done = false;
                        chunk
                    }
                    None => {
                        // Encode a zero length chunk
                        // the http1 encoder does the right thing
                        // encoding either the final chunk or ignoring the input
                        http::Chunk::from(Vec::new())
                    }
                });

                match encoder.encode(&mut self.io, wbuf.buf()) {
                    Ok(n) => {
                        wbuf.consume(n);

                        if !wbuf.is_written() {
                            trace!("Conn::start_send frame not written, queued");
                            *queued = Some(wbuf);
                        }
                    },
                    Err(e) => match e.kind() {
                        io::ErrorKind::WouldBlock => {
                            trace!("Conn::start_send frame not written, queued");
                            *queued = Some(wbuf);
                        },
                        _ => return Err(e)
                    }
                }

                if encoder.is_eof() {
                    Writing::KeepAlive
                } else if is_done {
                    Writing::Closed
                } else {
                    return Ok(AsyncSink::Ready);
                }
            },
            Writing::Init | Writing::KeepAlive | Writing::Closed => unreachable!(),
        };
        self.state.writing = state;
        Ok(AsyncSink::Ready)
    }

    fn write_queued(&mut self) -> Poll<(), io::Error> {
        trace!("Conn::write_queued()");
        match self.state.writing {
            Writing::Body(ref mut encoder, ref mut queued) => {
                let complete = if let Some(chunk) = queued.as_mut() {
                    let n = try_nb!(encoder.encode(&mut self.io, chunk.buf()));
                    chunk.consume(n);
                    chunk.is_written()
                } else {
                    true
                };
                trace!("Conn::write_queued complete = {}", complete);
                if complete {
                    *queued = None;
                    Ok(Async::Ready(()))
                } else {
                    Ok(Async::NotReady)
                }
            },
            _ => Ok(Async::Ready(())),
        }
    }

    fn flush(&mut self) -> Poll<(), io::Error> {
        let ret = try!(self.write_queued());
        try_nb!(self.io.flush());
        self.state.try_keep_alive();
        trace!("flushed {:?}", self.state);
        if self.is_read_ready() {
            ::futures::task::park().unpark();
        }
        Ok(ret)

    }
}

impl<I, T, K> Stream for Conn<I, T, K>
where I: Io,
      T: Http1Transaction,
      K: KeepAlive,
      T::Outgoing: fmt::Debug {
    type Item = Frame<http::MessageHead<T::Incoming>, http::Chunk, ::Error>;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        trace!("Conn::poll()");

        if self.is_read_closed() {
            trace!("Conn::poll when closed");
            Ok(Async::Ready(None))
        } else if self.can_read_head() {
            self.read_head()
        } else if self.can_read_body() {
            self.read_body()
                .map(|async| async.map(|chunk| Some(Frame::Body {
                    chunk: chunk
                })))
        } else {
            trace!("poll when on keep-alive");
            Ok(Async::NotReady)
        }
    }
}

impl<I, T, K> Sink for Conn<I, T, K>
where I: Io,
      T: Http1Transaction,
      K: KeepAlive,
      T::Outgoing: fmt::Debug {
    type SinkItem = Frame<http::MessageHead<T::Outgoing>, http::Chunk, ::Error>;
    type SinkError = io::Error;

    fn start_send(&mut self, frame: Self::SinkItem) -> StartSend<Self::SinkItem, Self::SinkError> {
        trace!("Conn::start_send( frame={:?} )", DebugFrame(&frame));

        let frame: Self::SinkItem = match frame {
            Frame::Message { message: head, body } => {
                if self.can_write_head() {
                    return self.write_head(head, body)
                        .map(|async| {
                            match async {
                                AsyncSink::Ready => AsyncSink::Ready,
                                AsyncSink::NotReady(head) => {
                                    AsyncSink::NotReady(Frame::Message {
                                        message: head,
                                        body: body,
                                    })
                                }
                            }
                        })
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
                } else if chunk.is_none() {
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

        error!("writing illegal frame; state={:?}, frame={:?}", self.state.writing, frame);
        Err(io::Error::new(io::ErrorKind::InvalidInput, "illegal frame"))

    }

    fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
        trace!("Conn::poll_complete()");
        let ret = self.flush();
        trace!("Conn::flush = {:?}", ret);
        ret
    }
}

impl<I, T, K> Transport for Conn<I, T, K>
where I: Io + 'static,
      T: Http1Transaction + 'static,
      K: KeepAlive + 'static,
      T::Outgoing: fmt::Debug {}

impl<I, T, K: fmt::Debug> fmt::Debug for Conn<I, T, K> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Conn")
            .field("state", &self.state)
            .field("io", &self.io)
            .finish()
    }
}

#[derive(Debug)]
struct State<K> {
    reading: Reading,
    writing: Writing,
    keep_alive: K,
}

#[derive(Debug)]
enum Reading {
    Init,
    Body(Decoder),
    KeepAlive,
    Closed,
}

#[derive(Debug)]
enum Writing {
    Init,
    Body(Encoder, Option<Cursor<http::Chunk>>),
    KeepAlive,
    Closed,
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
    Idle(Instant),
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
        *self = KA::Idle(Instant::now());
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

impl<K: KeepAlive> State<K> {
    fn close(&mut self) {
        trace!("State::close()");
        self.reading = Reading::Closed;
        self.writing = Writing::Closed;
        self.keep_alive.disable();
    }

    fn try_keep_alive(&mut self) {
        match (&self.reading, &self.writing) {
            (&Reading::KeepAlive, &Writing::KeepAlive) => {
                if let KA::Busy = self.keep_alive.status() {
                    self.reading = Reading::Init;
                    self.writing = Writing::Init;
                    self.keep_alive.idle();
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

    fn was_idle(&self) -> bool {
        if let KA::Idle(..) = self.keep_alive.status() {
            true
        } else {
            false
        }
    }

    fn busy(&mut self) {
        self.keep_alive.busy();
    }

    fn is_read_closed(&self) -> bool {
        match self.reading {
            Reading::Closed => true,
            _ => false
        }
    }

    #[allow(unused)]
    fn is_write_closed(&self) -> bool {
        match self.writing {
            Writing::Closed => true,
            _ => false
        }
    }
}

// The DebugFrame and DebugChunk are simple Debug implementations that allow
// us to dump the frame into logs, wihtout logging the entirety of the bytes.
struct DebugFrame<'a, T: fmt::Debug + 'a>(&'a Frame<http::MessageHead<T>, http::Chunk, ::Error>);

impl<'a, T: fmt::Debug + 'a> fmt::Debug for DebugFrame<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self.0 {
            Frame::Message { ref message, ref body } => {
                f.debug_struct("Message")
                    .field("message", message)
                    .field("body", body)
                    .finish()
            },
            Frame::Body { chunk: Some(ref chunk) } => {
                f.debug_struct("Body")
                    .field("chunk", &DebugChunk(chunk))
                    .finish()
            },
            Frame::Body { chunk: None } => {
                f.debug_struct("Body")
                    .field("chunk", &None::<()>)
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

struct DebugChunk<'a>(&'a http::Chunk);

impl<'a> fmt::Debug for DebugChunk<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("Chunk")
            .field(&self.0.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use futures::{Async, Stream, Sink};
    use tokio_proto::streaming::pipeline::Frame;

    use http::{MessageHead, ServerTransaction};
    use http::h1::Encoder;
    use mock::AsyncIo;

    use super::{Conn, Writing};
    use ::uri::Uri;

    #[test]
    fn test_conn_init_read() {
        let good_message = b"GET / HTTP/1.1\r\n\r\n".to_vec();
        let len = good_message.len();
        let io = AsyncIo::new_buf(good_message, len);
        let mut conn = Conn::<_, ServerTransaction>::new(io, Default::default());

        match conn.poll().unwrap() {
            Async::Ready(Some(Frame::Message { message, body: false })) => {
                assert_eq!(message, MessageHead {
                    subject: ::http::RequestLine(::Get, Uri::new("/").unwrap()),
                    .. MessageHead::default()
                })
            },
            f => panic!("frame is not Frame::Message: {:?}", f)
        }
    }

    #[test]
    fn test_conn_closed_read() {
        let io = AsyncIo::new_buf(vec![], 0);
        let mut conn = Conn::<_, ServerTransaction>::new(io, Default::default());
        conn.state.close();

        match conn.poll().unwrap() {
            Async::Ready(None) => {},
            other => panic!("frame is not None: {:?}", other)
        }
    }

    #[test]
    fn test_conn_body_write_length() {
        extern crate pretty_env_logger;
        use ::futures::Future;
        let _ = pretty_env_logger::init();
        let _: Result<(), ()> = ::futures::lazy(|| {
            let io = AsyncIo::new_buf(vec![], 0);
            let mut conn = Conn::<_, ServerTransaction>::new(io, Default::default());
            let max = ::http::io::MAX_BUFFER_SIZE + 4096;
            conn.state.writing = Writing::Body(Encoder::length((max * 2) as u64), None);

            assert!(conn.start_send(Frame::Body { chunk: Some(vec![b'a'; 1024 * 4].into()) }).unwrap().is_ready());
            match conn.state.writing {
                Writing::Body(_, None) => {},
                _ => panic!("writing did not queue chunk: {:?}", conn.state.writing),
            }

            assert!(conn.start_send(Frame::Body { chunk: Some(vec![b'b'; max].into()) }).unwrap().is_ready());

            match conn.state.writing {
                Writing::Body(_, Some(_)) => {},
                _ => panic!("writing did not queue chunk: {:?}", conn.state.writing),
            }

            assert!(conn.start_send(Frame::Body { chunk: Some(vec![b'b'; 1024 * 4].into()) }).unwrap().is_not_ready());

            conn.io.io_mut().block_in(1024 * 3);
            assert!(conn.poll_complete().unwrap().is_not_ready());
            conn.io.io_mut().block_in(1024 * 3);
            assert!(conn.poll_complete().unwrap().is_not_ready());
            conn.io.io_mut().block_in(max * 2);
            assert!(conn.poll_complete().unwrap().is_not_ready());
            assert!(conn.poll_complete().unwrap().is_ready());

            assert!(conn.start_send(Frame::Body { chunk: Some(vec![b'c'; 1024 * 4].into()) }).unwrap().is_ready());
            Ok(())
        }).wait();
    }

    #[test]
    fn test_conn_body_write_chunked() {
        use ::futures::Future;
        let _: Result<(), ()> = ::futures::lazy(|| {
            let io = AsyncIo::new_buf(vec![], 4096);
            let mut conn = Conn::<_, ServerTransaction>::new(io, Default::default());
            conn.state.writing = Writing::Body(Encoder::chunked(), None);

            assert!(conn.start_send(Frame::Body { chunk: Some("headers".into()) }).unwrap().is_ready());
            assert!(conn.start_send(Frame::Body { chunk: Some(vec![b'x'; 4096].into()) }).unwrap().is_ready());
            Ok(())
        }).wait();
    }
    #[test]
    fn test_conn_closed_write() {
        let io = AsyncIo::new_buf(vec![], 0);
        let mut conn = Conn::<_, ServerTransaction>::new(io, Default::default());
        conn.state.close();

        match conn.start_send(Frame::Body { chunk: Some(b"foobar".to_vec().into()) }) {
            Err(_e) => {},
            other => panic!("did not return Err: {:?}", other)
        }

        assert!(conn.state.is_write_closed());
    }
}
