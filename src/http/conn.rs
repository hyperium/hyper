use std::borrow::Cow;
use std::fmt;
use std::hash::Hash;
use std::io::{self, Write};
use std::marker::PhantomData;
use std::mem;
use std::time::Duration;

use futures::{Poll, Async};
use tokio::io::{Io, FramedIo};
use tokio_proto::pipeline::Frame;

use header::{ContentLength, TransferEncoding};
use http::{self, h1, Http1Transaction, IoBuf, WriteBuf};
use http::h1::{Encoder, Decoder};
use http::buffer::Buffer;
use version::HttpVersion;


/// This handles a connection, which will have been established over a
/// Transport (like a socket), and will likely include multiple
/// `Transaction`s over HTTP.
///
/// The connection will determine when a message begins and ends, creating
/// a new message `TransactionHandler` for each one, as well as determine if this
/// connection can be kept alive after the message, or if it is complete.
pub struct Conn<I, T> {
    io: IoBuf<I>,
    keep_alive_enabled: bool,
    state: State,
    _marker: PhantomData<T>
}

impl<I, T> Conn<I, T> {
    pub fn new(transport: I) -> Conn<I, T> {
        Conn {
            io: IoBuf {
                read_buf: Buffer::new(),
                write_buf: Buffer::new(),
                transport: transport,
            },
            keep_alive_enabled: true,
            state: State {
                reading: Reading::Init,
                writing: Writing::Init,
                keep_alive: true,
            },
            _marker: PhantomData,
        }
    }
}

impl<I: Io, T: Http1Transaction> Conn<I, T> {

    fn parse(&mut self) -> ::Result<Option<http::MessageHead<T::Incoming>>> {
        self.io.parse::<T>()
    }

    fn is_read_closed(&self) -> bool {
        self.state.is_read_closed()
    }

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

    fn read_head(&mut self) -> Poll<Frame<http::MessageHead<T::Incoming>, http::Chunk, ::Error>, io::Error> {
        debug_assert!(self.can_read_head());
        trace!("Conn::read_head");

        let (version, head) = match self.parse() {
            Ok(Some(head)) => (head.version, head),
            Ok(None) => return Ok(Async::NotReady),
            Err(e) => {
                self.state.close();
                self.io.read_buf.consume_leading_lines();
                if !self.io.read_buf.is_empty() {
                    error!("parse error ({}) with bytes: {:?}", e, self.io.read_buf.bytes());
                    return Ok(Async::Ready(Frame::Error { error: e }));
                } else {
                    trace!("parse error with 0 input, err = {:?}", e);
                    return Ok(Async::Ready(Frame::Done));
                }
            }
        };

        match version {
            HttpVersion::Http10 | HttpVersion::Http11 => {
                let decoder = match T::decoder(&head) {
                    Ok(d) => d,
                    Err(e) => {
                        error!("decoder error = {:?}", e);
                        self.state.close();
                        return Ok(Async::Ready(Frame::Error { error: e }));
                    }
                };
                let wants_keep_alive = http::should_keep_alive(version, &head.headers);
                self.state.keep_alive &= wants_keep_alive;
                let (body, reading) = if decoder.is_eof() {
                    (false, Reading::KeepAlive)
                } else {
                    (true, Reading::Body(decoder))
                };
                self.state.reading = reading;
                return Ok(Async::Ready(Frame::Message { message: head, body: body }));
            },
            _ => {
                error!("unimplemented HTTP Version = {:?}", version);
                self.state.close();
                return Ok(Async::Ready(Frame::Error { error: ::Error::Version }));
            }
        }
    }

    fn read_body(&mut self) -> Poll<Option<http::Chunk>, io::Error> {
        debug_assert!(self.can_read_body());

        trace!("Conn::read_body");

        let (reading, ret) = match self.state.reading {
            Reading::Body(ref mut decoder) => {
                //TODO use an appendbuf or something
                let mut buf = vec![0; 1024 * 4];
                let n = try_nb!(decoder.decode(&mut self.io, &mut buf));
                if n > 0 {
                    buf.truncate(n);
                    return Ok(Async::Ready(Some(http::Chunk::from(buf))));
                } else {
                    if decoder.is_eof() {
                        //TODO: should be Reading::KeepAlive
                        (Reading::Closed, Ok(Async::Ready(None)))
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

    fn write_head(&mut self, mut head: http::MessageHead<T::Outgoing>, body: bool) -> Poll<(), io::Error> {
        debug_assert!(self.can_write_head());
        if !body {
            head.headers.remove::<TransferEncoding>();
            //TODO: check that this isn't a response to a HEAD
            //request, which could include the content-length
            //even if no body is to be written
            head.headers.set(ContentLength(0));
        }

        let wants_keep_alive = http::should_keep_alive(head.version, &head.headers);
        self.state.keep_alive &= wants_keep_alive;
        let mut buf = Vec::new();
        let encoder = T::encode(&mut head, &mut buf);
        self.io.write(&buf).unwrap();
        self.state.writing = if body {
            Writing::Body(encoder)
        } else {
            Writing::KeepAlive
        };

        Ok(Async::Ready(()))
    }

    fn write_body(&mut self, chunk: Option<http::Chunk>) -> Poll<(), io::Error> {
        debug_assert!(self.can_write_body());

        let state = match self.state.writing {
            Writing::Body(ref mut encoder) => {
                let mut is_done = true;
                if let Some(chunk) = chunk {
                    is_done = false;
                    // TODO: this needs to check our write_buf can receive this
                    // chunk, and if not, shove it into `self` and be NotReady
                    // until we've flushed and fit the cached chunk
                    try_nb!(encoder.encode(&mut self.io, &chunk));
                }
                if encoder.is_eof() {
                    Writing::KeepAlive
                } else if is_done {
                    Writing::Closed
                } else {
                    return Ok(Async::Ready(()));
                }
            },
            Writing::Init | Writing::KeepAlive | Writing::Closed => unreachable!(),
        };
        self.state.writing = state;
        Ok(Async::Ready(()))
    }

}

impl<I, T> FramedIo for Conn<I, T>
where I: Io,
      T: Http1Transaction,
      T::Outgoing: fmt::Debug {
    type In = Frame<http::MessageHead<T::Outgoing>, http::Chunk, ::Error>;
    type Out = Frame<http::MessageHead<T::Incoming>, http::Chunk, ::Error>;

    fn poll_read(&mut self) -> Async<()> {
        let ret = match self.state.reading {
            Reading::Closed => Async::Ready(()),
            Reading::KeepAlive => Async::NotReady,
            _ => self.io.transport.poll_read()
        };
        trace!("Conn::poll_read = {:?}", ret);
        ret
    }

    fn read(&mut self) -> Poll<Self::Out, io::Error> {
        trace!("Conn::read");

        if self.is_read_closed() {
            trace!("Conn::read when closed");
            Ok(Async::Ready(Frame::Done))
        } else if self.can_read_head() {
            self.read_head()
        } else if self.can_read_body() {
            self.read_body().map(|async| async.map(|chunk| Frame::Body { chunk: chunk }))
        } else {
            trace!("read when on keep-alive");
            Ok(Async::NotReady)
        }
    }

    fn poll_write(&mut self) -> Async<()> {
        trace!("Conn::poll_write");
        //self.io.transport.poll_write()
        Async::Ready(())
    }

    fn write(&mut self, frame: Self::In) -> Poll<(), io::Error> {
        trace!("Conn::write frame={:?}", frame);

        let frame: Self::In = match frame {
            Frame::Message { message: head, body } => {
                if self.can_write_head() {
                    return self.write_head(head, body);
                } else {
                    Frame::Message { message: head, body: body }
                }
            },
            Frame::Body { chunk } => {
                if self.can_write_body() {
                    return self.write_body(chunk);
                } else if chunk.is_none() {
                    return Ok(Async::Ready(()));
                } else {
                    Frame::Body { chunk: chunk }
                }
            },
            Frame::Error { error } => {
                self.state.close();
                return Ok(Async::Ready(()));
            },
            Frame::Done => {
                self.state.writing = Writing::Closed;
                return Ok(Async::Ready(()));
            }
        };

        error!("writing illegal frame; state={:?}, frame={:?}", self.state.writing, frame);
        Err(io::Error::new(io::ErrorKind::InvalidInput, "illegal frame"))
    }

    fn flush(&mut self) -> Poll<(), io::Error> {
        let ret = match self.io.flush() {
            Ok(()) => {
                self.state.try_keep_alive();
                trace!("flushed {:?}", self.state);
                if !self.is_read_closed() {
                    if self.poll_read().is_ready() {
                        ::futures::task::park().unpark();
                    }
                }
                Ok(Async::Ready(()))
            },
            Err(e) => match e.kind() {
                io::ErrorKind::WouldBlock => Ok(Async::NotReady),
                _ => Err(e)
            }
        };
        trace!("Conn::flush = {:?}", ret);
        ret
    }
}

impl<I, T> fmt::Debug for Conn<I, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Conn")
            .field("keep_alive_enabled", &self.keep_alive_enabled)
            .field("state", &self.state)
            .field("io", &self.io)
            .finish()
    }
}

#[derive(Debug)]
struct State {
    reading: Reading,
    writing: Writing,
    keep_alive: bool,
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
    Body(Encoder),
    KeepAlive,
    Closed,
}

impl State {
    fn close(&mut self) {
        trace!("State::close");
        self.reading = Reading::Closed;
        self.writing = Writing::Closed;
    }

    fn try_keep_alive(&mut self) {
        match (&self.reading, &self.writing) {
            (&Reading::KeepAlive, &Writing::KeepAlive) => {
                if self.keep_alive {
                    self.reading = Reading::Init;
                    self.writing = Writing::Init;
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
mod tests {
    use futures::Async;
    use tokio::io::FramedIo;
    use tokio_proto::pipeline::Frame;

    use http::{MessageHead, ServerTransaction};
    use http::h1::Encoder;
    use mock::AsyncIo;

    use super::{Conn, State, Writing};

    #[test]
    fn test_conn_init_read() {
        let good_message = b"GET / HTTP/1.1\r\n\r\n".to_vec();
        let len = good_message.len();
        let io = AsyncIo::new_buf(good_message, len);
        let mut conn = Conn::<_, ServerTransaction>::new(io);

        match conn.read().unwrap() {
            Async::Ready(Frame::Message { message, body: false }) => {
                assert_eq!(message, MessageHead {
                    subject: ::http::RequestLine(::Get, ::RequestUri::AbsolutePath {
                        path: "/".to_string(),
                        query: None,
                    }),
                    .. MessageHead::default()
                })
            },
            f => panic!("frame is not Frame::Message: {:?}", f)
        }
    }

    #[test]
    fn test_conn_closed_read() {
        let io = AsyncIo::new_buf(vec![], 0);
        let mut conn = Conn::<_, ServerTransaction>::new(io);
        conn.state.close();

        match conn.read().unwrap() {
            Async::Ready(Frame::Done) => {},
            other => panic!("frame is not Frame::Done: {:?}", other)
        }
    }

    #[test]
    fn test_conn_body_write() {
        let io = AsyncIo::new_buf(vec![], 0);
        let mut conn = Conn::<_, ServerTransaction>::new(io);
        conn.state.writing = Writing::Body(Encoder::length(1024 * 5));

        match conn.write(Frame::Body { chunk: Some(vec![b'a'; 1024 * 4].into()) }) {
            Ok(Async::Ready(())) => {},
            other => panic!("did not return Ready: {:?}", other)
        }

        match conn.write(Frame::Body { chunk: Some(vec![b'b'; 1024 * 4].into()) }) {
            Ok(Async::NotReady) => {},
            other => panic!("did not return NotReady: {:?}", other)
        }

        assert!(conn.poll_write().is_not_ready(), "poll_write should not be ready");

        conn.io.transport.block_in(1024 * 3);
        assert!(conn.flush().unwrap().is_not_ready());
        assert!(conn.poll_write().is_not_ready(), "poll_write should not be ready");
        conn.io.transport.block_in(1024 * 3);
        assert!(conn.flush().unwrap().is_not_ready());
        assert!(conn.poll_write().is_not_ready(), "poll_write should not be ready");
        conn.io.transport.block_in(1024 * 3);
        assert!(conn.flush().unwrap().is_ready());
        assert!(conn.poll_write().is_ready(), "poll_write should be ready");
    }

    #[test]
    fn test_conn_closed_write() {
        let io = AsyncIo::new_buf(vec![], 0);
        let mut conn = Conn::<_, ServerTransaction>::new(io);
        conn.state.close();

        match conn.write(Frame::Body { chunk: Some(b"foobar".to_vec().into()) }) {
            Err(e) => {},
            other => panic!("did not return Err: {:?}", other)
        }

        assert!(conn.state.is_write_closed());
    }
}
