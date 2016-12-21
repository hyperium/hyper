//! Pieces pertaining to the HTTP message protocol.
use std::borrow::Cow;
use std::fmt;
use std::io::{self, Read, Write};

use header::Connection;
use header::ConnectionOption::{KeepAlive, Close};
use header::Headers;
use method::Method;
use status::StatusCode;
use uri::RequestUri;
use version::HttpVersion;
use version::HttpVersion::{Http10, Http11};

#[cfg(feature = "serde-serialization")]
use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub use self::conn::Conn;
pub use self::chunk::Chunk;

mod buffer;
mod chunk;
mod conn;
pub mod h1;
//mod h2;

macro_rules! nonblocking {
    ($e:expr) => ({
        match $e {
            Ok(n) => Ok(Some(n)),
            Err(e) => match e.kind() {
                io::ErrorKind::WouldBlock => Ok(None),
                _ => Err(e)
            }
        }
    });
}

#[derive(Clone)]
pub struct WriteBuf<T: AsRef<[u8]>> {
    bytes: T,
    pos: usize,
}

impl<T: AsRef<[u8]>> WriteBuf<T> {
    pub fn new(bytes: T) -> WriteBuf<T> {
        WriteBuf {
            bytes: bytes,
            pos: 0,
        }
    }

    pub fn is_written(&self) -> bool {
        trace!("WriteBuf::is_written pos = {}, len = {}", self.pos, self.bytes.as_ref().len());
        self.pos >= self.bytes.as_ref().len()
    }

    /*
    pub fn write_to<W: Write>(&mut self, dst: &mut W) -> io::Result<usize> {
        dst.write(&self.bytes.as_ref()[self.pos..]).map(|n| {
            self.pos += n;
            n
        })
    }
    */

    #[inline]
    pub fn buf(&self) -> &[u8] {
        &self.bytes.as_ref()[self.pos..]
    }

    #[inline]
    pub fn consume(&mut self, num: usize) {
        trace!("WriteBuf::consume({})", num);
        self.pos = ::std::cmp::min(self.bytes.as_ref().len(), self.pos + num);
    }
}

impl<T: AsRef<[u8]>> fmt::Debug for WriteBuf<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let bytes = self.buf();
        let reasonable_max = ::std::cmp::min(bytes.len(), 32);
        write!(f, "WriteBuf({:?})", &bytes[..reasonable_max])
    }
}

pub trait AtomicWrite {
    fn write_atomic(&mut self, data: &[&[u8]]) -> io::Result<usize>;
}

/*
#[cfg(not(windows))]
impl<T: Write + ::vecio::Writev> AtomicWrite for T {

    fn write_atomic(&mut self, bufs: &[&[u8]]) -> io::Result<usize> {
        self.writev(bufs)
    }

}

#[cfg(windows)]
*/
impl<T: Write> AtomicWrite for T {
    fn write_atomic(&mut self, bufs: &[&[u8]]) -> io::Result<usize> {
        if cfg!(not(windows)) {
            warn!("write_atomic not using writev");
        }
        let vec = bufs.concat();
        self.write(&vec)
    }
}
//}

pub struct IoBuf<T> {
    read_buf: self::buffer::Buffer,
    write_buf: self::buffer::Buffer,
    transport: T,
}

impl<T> fmt::Debug for IoBuf<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("IoBuf")
            .field("read_buf", &self.read_buf)
            .field("write_buf", &self.write_buf)
            .finish()
    }
}

impl<T: Read> Read for IoBuf<T> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        trace!("IoBuf.read self={}, buf={}", self.read_buf.len(), buf.len());
        let n = try!(self.read_buf.bytes().read(buf));
        self.read_buf.consume(n);
        if n == 0 {
            self.read_buf.reset();
            self.transport.read(&mut buf[n..])
        } else {
            Ok(n)
        }
    }
}

impl<T: Write> Write for IoBuf<T> {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        Ok(self.write_buf.write(data))
    }

    fn flush(&mut self) -> io::Result<()> {
        self.write_buf.write_into(&mut self.transport).and_then(|_n| {
            if self.write_buf.is_empty() {
                Ok(())
            } else {
                Err(io::Error::new(io::ErrorKind::WouldBlock, "wouldblock"))
            }
        })
    }
}

impl<T: Read> IoBuf<T> {
    fn parse<S: Http1Transaction>(&mut self) -> ::Result<Option<MessageHead<S::Incoming>>> {
        match self.read_buf.read_from(&mut self.transport) {
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
        match try!(parse::<S, _>(self.read_buf.bytes())) {
            Some((head, len)) => {
                trace!("parsed {} bytes out of {}", len, self.read_buf.len());
                self.read_buf.consume(len);
                Ok(Some(head))
            },
            None => {
                if self.read_buf.is_max_size() {
                    debug!("MAX_BUFFER_SIZE reached, closing");
                    Err(::Error::TooLarge)
                } else {
                    Ok(None)
                }
            },
        }
    }
}

/// An Incoming Message head. Includes request/status line, and headers.
#[derive(Debug, Default, PartialEq)]
pub struct MessageHead<S> {
    /// HTTP version of the message.
    pub version: HttpVersion,
    /// Subject (request line or status line) of Incoming message.
    pub subject: S,
    /// Headers of the Incoming message.
    pub headers: Headers
}

/// An incoming request message.
pub type RequestHead = MessageHead<RequestLine>;

#[derive(Debug, Default, PartialEq)]
pub struct RequestLine(pub Method, pub RequestUri);

impl fmt::Display for RequestLine {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {}", self.0, self.1)
    }
}

/// An incoming response message.
pub type ResponseHead = MessageHead<RawStatus>;

impl<S> MessageHead<S> {
    pub fn should_keep_alive(&self) -> bool {
        should_keep_alive(self.version, &self.headers)
    }
}

/// The raw status code and reason-phrase.
#[derive(Clone, PartialEq, Debug)]
pub struct RawStatus(pub u16, pub Cow<'static, str>);

impl fmt::Display for RawStatus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {}", self.0, self.1)
    }
}

impl From<StatusCode> for RawStatus {
    fn from(status: StatusCode) -> RawStatus {
        RawStatus(status.to_u16(), Cow::Borrowed(status.canonical_reason().unwrap_or("")))
    }
}

impl Default for RawStatus {
    fn default() -> RawStatus {
        RawStatus(200, Cow::Borrowed("OK"))
    }
}

#[cfg(feature = "serde-serialization")]
impl Serialize for RawStatus {
    fn serialize<S>(&self, serializer: &mut S) -> Result<(), S::Error> where S: Serializer {
        (self.0, &self.1).serialize(serializer)
    }
}

#[cfg(feature = "serde-serialization")]
impl Deserialize for RawStatus {
    fn deserialize<D>(deserializer: &mut D) -> Result<RawStatus, D::Error> where D: Deserializer {
        let representation: (u16, String) = try!(Deserialize::deserialize(deserializer));
        Ok(RawStatus(representation.0, Cow::Owned(representation.1)))
    }
}

impl From<MessageHead<::StatusCode>> for MessageHead<RawStatus> {
    fn from(head: MessageHead<::StatusCode>) -> MessageHead<RawStatus> {
        MessageHead {
            subject: head.subject.into(),
            version: head.version,
            headers: head.headers,
        }
    }
}

/// Checks if a connection should be kept alive.
#[inline]
pub fn should_keep_alive(version: HttpVersion, headers: &Headers) -> bool {
    let ret = match (version, headers.get::<Connection>()) {
        (Http10, None) => false,
        (Http10, Some(conn)) if !conn.contains(&KeepAlive) => false,
        (Http11, Some(conn)) if conn.contains(&Close)  => false,
        _ => true
    };
    trace!("should_keep_alive(version={:?}, header={:?}) = {:?}", version, headers.get::<Connection>(), ret);
    ret
}

pub type ParseResult<T> = ::Result<Option<(MessageHead<T>, usize)>>;

pub fn parse<T: Http1Transaction<Incoming=I>, I>(rdr: &[u8]) -> ParseResult<I> {
    h1::parse::<T, I>(rdr)
}

#[derive(Debug)]
pub enum ServerTransaction {}

#[derive(Debug)]
pub enum ClientTransaction {}

pub trait Http1Transaction {
    type Incoming;
    type Outgoing: Default;
    fn parse(bytes: &[u8]) -> ParseResult<Self::Incoming>;
    fn decoder(head: &MessageHead<Self::Incoming>) -> ::Result<h1::Decoder>;
    fn encode(head: &mut MessageHead<Self::Outgoing>, dst: &mut Vec<u8>) -> h1::Encoder;
    fn should_set_length(head: &MessageHead<Self::Outgoing>) -> bool;
}


#[test]
fn test_should_keep_alive() {
    let mut headers = Headers::new();

    assert!(!should_keep_alive(Http10, &headers));
    assert!(should_keep_alive(Http11, &headers));

    headers.set(Connection::close());
    assert!(!should_keep_alive(Http10, &headers));
    assert!(!should_keep_alive(Http11, &headers));

    headers.set(Connection::keep_alive());
    assert!(should_keep_alive(Http10, &headers));
    assert!(should_keep_alive(Http11, &headers));
}

#[test]
fn test_iobuf_write_empty_slice() {
    use mock::{AsyncIo, Buf};

    let mut mock = AsyncIo::new(Buf::new(), 256);
    mock.error(io::Error::new(io::ErrorKind::Other, "logic error"));

    let mut io_buf = IoBuf {
        write_buf: Default::default(),
        read_buf: Default::default(),
        transport: mock,
    };

    // underlying io will return the logic error upon write,
    // so we are testing that the io_buf does not trigger a write
    // when there is nothing to flush
    io_buf.flush().expect("should short-circuit flush");
}
