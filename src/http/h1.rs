//! Adapts the HTTP/1.1 implementation into the `HttpMessage` API.
use std::borrow::Cow;
use std::cmp::min;
use std::fmt;
use std::io::{self, Write, BufWriter, BufRead, Read};
use std::net::Shutdown;
use std::time::Duration;

use httparse;
use url::Position as UrlPosition;

use buffer::BufReader;
use Error;
use header::{Headers, ContentLength, TransferEncoding};
use header::Encoding::Chunked;
use method::{Method};
use net::{NetworkConnector, NetworkStream};
use status::StatusCode;
use version::HttpVersion;
use version::HttpVersion::{Http10, Http11};
use uri::RequestUri;

use self::HttpReader::{SizedReader, ChunkedReader, EofReader, EmptyReader};
use self::HttpWriter::{ChunkedWriter, SizedWriter, EmptyWriter, ThroughWriter};

use http::{
    RawStatus,
    Protocol,
    HttpMessage,
    RequestHead,
    ResponseHead,
};
use header;
use version;

const MAX_INVALID_RESPONSE_BYTES: usize = 1024 * 128;

#[derive(Debug)]
struct Wrapper<T> {
    obj: Option<T>,
}

impl<T> Wrapper<T> {
    pub fn new(obj: T) -> Wrapper<T> {
        Wrapper { obj: Some(obj) }
    }

    pub fn map_in_place<F>(&mut self, f: F) where F: FnOnce(T) -> T {
        let obj = self.obj.take().unwrap();
        let res = f(obj);
        self.obj = Some(res);
    }

    pub fn into_inner(self) -> T { self.obj.unwrap() }
    pub fn as_mut(&mut self) -> &mut T { self.obj.as_mut().unwrap() }
    pub fn as_ref(&self) -> &T { self.obj.as_ref().unwrap() }
}

#[derive(Debug)]
enum Stream {
    Idle(Box<NetworkStream + Send>),
    Writing(HttpWriter<BufWriter<Box<NetworkStream + Send>>>),
    Reading(HttpReader<BufReader<Box<NetworkStream + Send>>>),
}

impl Stream {
    fn writer_mut(&mut self) -> Option<&mut HttpWriter<BufWriter<Box<NetworkStream + Send>>>> {
        match *self {
            Stream::Writing(ref mut writer) => Some(writer),
            _ => None,
        }
    }
    fn reader_mut(&mut self) -> Option<&mut HttpReader<BufReader<Box<NetworkStream + Send>>>> {
        match *self {
            Stream::Reading(ref mut reader) => Some(reader),
            _ => None,
        }
    }
    fn reader_ref(&self) -> Option<&HttpReader<BufReader<Box<NetworkStream + Send>>>> {
        match *self {
            Stream::Reading(ref reader) => Some(reader),
            _ => None,
        }
    }

    fn new(stream: Box<NetworkStream + Send>) -> Stream {
        Stream::Idle(stream)
    }
}

/// An implementation of the `HttpMessage` trait for HTTP/1.1.
#[derive(Debug)]
pub struct Http11Message {
    is_proxied: bool,
    method: Option<Method>,
    stream: Wrapper<Stream>,
}

impl Write for Http11Message {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self.stream.as_mut().writer_mut() {
            None => Err(io::Error::new(io::ErrorKind::Other,
                                          "Not in a writable state")),
            Some(ref mut writer) => writer.write(buf),
        }
    }
    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        match self.stream.as_mut().writer_mut() {
            None => Err(io::Error::new(io::ErrorKind::Other,
                                          "Not in a writable state")),
            Some(ref mut writer) => writer.flush(),
        }
    }
}

impl Read for Http11Message {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self.stream.as_mut().reader_mut() {
            None => Err(io::Error::new(io::ErrorKind::Other,
                                          "Not in a readable state")),
            Some(ref mut reader) => reader.read(buf),
        }
    }
}

impl HttpMessage for Http11Message {
    fn set_outgoing(&mut self, mut head: RequestHead) -> ::Result<RequestHead> {
        let mut res = Err(Error::from(io::Error::new(
                            io::ErrorKind::Other,
                            "")));
        let mut method = None;
        let is_proxied = self.is_proxied;
        self.stream.map_in_place(|stream: Stream| -> Stream {
            let stream = match stream {
                Stream::Idle(stream) => stream,
                _ => {
                    res = Err(Error::from(io::Error::new(
                                io::ErrorKind::Other,
                                "Message not idle, cannot start new outgoing")));
                    return stream;
                },
            };
            let mut stream = BufWriter::new(stream);

            {
                let uri = if is_proxied {
                    head.url.as_ref()
                } else {
                    &head.url[UrlPosition::BeforePath..UrlPosition::AfterQuery]
                };

                let version = version::HttpVersion::Http11;
                debug!("request line: {:?} {:?} {:?}", head.method, uri, version);
                match write!(&mut stream, "{} {} {}{}",
                             head.method, uri, version, LINE_ENDING) {
                                 Err(e) => {
                                     res = Err(From::from(e));
                                     // TODO What should we do if the BufWriter doesn't wanna
                                     // relinquish the stream?
                                     return Stream::Idle(stream.into_inner().ok().unwrap());
                                 },
                                 Ok(_) => {},
                             };
            }

            let stream = {
                let write_headers = |mut stream: BufWriter<Box<NetworkStream + Send>>, head: &RequestHead| {
                    debug!("headers={:?}", head.headers);
                    match write!(&mut stream, "{}{}", head.headers, LINE_ENDING) {
                        Ok(_) => Ok(stream),
                        Err(e) => {
                            Err((e, stream.into_inner().unwrap()))
                        }
                    }
                };
                match head.method {
                    Method::Get | Method::Head => {
                        let writer = match write_headers(stream, &head) {
                            Ok(w) => w,
                            Err(e) => {
                                res = Err(From::from(e.0));
                                return Stream::Idle(e.1);
                            }
                        };
                        EmptyWriter(writer)
                    },
                    _ => {
                        let mut chunked = true;
                        let mut len = 0;

                        match head.headers.get::<header::ContentLength>() {
                            Some(cl) => {
                                chunked = false;
                                len = **cl;
                            },
                            None => ()
                        };

                        // can't do in match above, thanks borrowck
                        if chunked {
                            let encodings = match head.headers.get_mut::<header::TransferEncoding>() {
                                Some(encodings) => {
                                    //TODO: check if chunked is already in encodings. use HashSet?
                                    encodings.push(header::Encoding::Chunked);
                                    false
                                },
                                None => true
                            };

                            if encodings {
                                head.headers.set(
                                    header::TransferEncoding(vec![header::Encoding::Chunked]))
                            }
                        }

                        let stream = match write_headers(stream, &head) {
                            Ok(s) => s,
                            Err(e) => {
                                res = Err(From::from(e.0));
                                return Stream::Idle(e.1);
                            },
                        };

                        if chunked {
                            ChunkedWriter(stream)
                        } else {
                            SizedWriter(stream, len)
                        }
                    }
                }
            };

            method = Some(head.method.clone());
            res = Ok(head);
            Stream::Writing(stream)
        });

        self.method = method;
        res
    }

    fn get_incoming(&mut self) -> ::Result<ResponseHead> {
        try!(self.flush_outgoing());
        let method = self.method.take().unwrap_or(Method::Get);
        let mut res = Err(From::from(
                        io::Error::new(io::ErrorKind::Other,
                        "Read already in progress")));
        self.stream.map_in_place(|stream| {
            let stream = match stream {
                Stream::Idle(stream) => stream,
                _ => {
                    // The message was already in the reading state...
                    // TODO Decide what happens in case we try to get a new incoming at that point
                    res = Err(From::from(
                            io::Error::new(io::ErrorKind::Other,
                                           "Read already in progress")));
                    return stream;
                }
            };

            let expected_no_content = stream.previous_response_expected_no_content();
            trace!("previous_response_expected_no_content = {}", expected_no_content);

            let mut stream = BufReader::new(stream);

            let mut invalid_bytes_read = 0;
            let head;
            loop {
                head = match parse_response(&mut stream) {
                    Ok(head) => head,
                    Err(::Error::Version)
                        if expected_no_content && invalid_bytes_read < MAX_INVALID_RESPONSE_BYTES => {
                            trace!("expected_no_content, found content");
                            invalid_bytes_read += 1;
                            stream.consume(1);
                            continue;
                        }
                    Err(e) => {
                        res = Err(e);
                        return Stream::Idle(stream.into_inner());
                    }
                };
                break;
            }

            let raw_status = head.subject;
            let headers = head.headers;

            let is_empty = !should_have_response_body(&method, raw_status.0);
            stream.get_mut().set_previous_response_expected_no_content(is_empty);
            // According to https://tools.ietf.org/html/rfc7230#section-3.3.3
            // 1. HEAD reponses, and Status 1xx, 204, and 304 cannot have a body.
            // 2. Status 2xx to a CONNECT cannot have a body.
            // 3. Transfer-Encoding: chunked has a chunked body.
            // 4. If multiple differing Content-Length headers or invalid, close connection.
            // 5. Content-Length header has a sized body.
            // 6. Not Client.
            // 7. Read till EOF.
            let reader = if is_empty {
                EmptyReader(stream)
            } else {
                if let Some(&TransferEncoding(ref codings)) = headers.get() {
                    if codings.last() == Some(&Chunked) {
                        ChunkedReader(stream, None)
                    } else {
                        trace!("not chuncked. read till eof");
                        EofReader(stream)
                    }
                } else if let Some(&ContentLength(len)) =  headers.get() {
                    SizedReader(stream, len)
                } else if headers.has::<ContentLength>() {
                    trace!("illegal Content-Length: {:?}", headers.get_raw("Content-Length"));
                    res = Err(Error::Header);
                    return Stream::Idle(stream.into_inner());
                } else {
                    trace!("neither Transfer-Encoding nor Content-Length");
                    EofReader(stream)
                }
            };

            trace!("Http11Message.reader = {:?}", reader);


            res = Ok(ResponseHead {
                headers: headers,
                raw_status: raw_status,
                version: head.version,
            });

            Stream::Reading(reader)
        });
        res
    }

    fn has_body(&self) -> bool {
        match self.stream.as_ref().reader_ref() {
            Some(&EmptyReader(..)) |
            Some(&SizedReader(_, 0)) |
            Some(&ChunkedReader(_, Some(0))) => false,
            // specifically EofReader is always true
            _ => true
        }
    }

    #[inline]
    fn set_read_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        self.get_ref().set_read_timeout(dur)
    }

    #[inline]
    fn set_write_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        self.get_ref().set_write_timeout(dur)
    }

    #[inline]
    fn close_connection(&mut self) -> ::Result<()> {
        try!(self.get_mut().close(Shutdown::Both));
        Ok(())
    }

    #[inline]
    fn set_proxied(&mut self, val: bool) {
        self.is_proxied = val;
    }
}

impl Http11Message {
    /// Consumes the `Http11Message` and returns the underlying `NetworkStream`.
    pub fn into_inner(self) -> Box<NetworkStream + Send> {
        match self.stream.into_inner() {
            Stream::Idle(stream) => stream,
            Stream::Writing(stream) => stream.into_inner().into_inner().unwrap(),
            Stream::Reading(stream) => stream.into_inner().into_inner(),
        }
    }

    /// Gets a borrowed reference to the underlying `NetworkStream`, regardless of the state of the
    /// `Http11Message`.
    pub fn get_ref(&self) -> &(NetworkStream + Send) {
        match *self.stream.as_ref() {
            Stream::Idle(ref stream) => &**stream,
            Stream::Writing(ref stream) => &**stream.get_ref().get_ref(),
            Stream::Reading(ref stream) => &**stream.get_ref().get_ref()
        }
    }

    /// Gets a mutable reference to the underlying `NetworkStream`, regardless of the state of the
    /// `Http11Message`.
    pub fn get_mut(&mut self) -> &mut (NetworkStream + Send) {
        match *self.stream.as_mut() {
            Stream::Idle(ref mut stream) => &mut **stream,
            Stream::Writing(ref mut stream) => &mut **stream.get_mut().get_mut(),
            Stream::Reading(ref mut stream) => &mut **stream.get_mut().get_mut()
        }
    }

    /// Creates a new `Http11Message` that will use the given `NetworkStream` for communicating to
    /// the peer.
    pub fn with_stream(stream: Box<NetworkStream + Send>) -> Http11Message {
        Http11Message {
            is_proxied: false,
            method: None,
            stream: Wrapper::new(Stream::new(stream)),
        }
    }

    /// Flushes the current outgoing content and moves the stream into the `stream` property.
    ///
    /// TODO It might be sensible to lift this up to the `HttpMessage` trait itself...
    pub fn flush_outgoing(&mut self) -> ::Result<()> {
        let mut res = Ok(());
        self.stream.map_in_place(|stream| {
            let writer = match stream {
                Stream::Writing(writer) => writer,
                _ => {
                    res = Ok(());
                    return stream;
                },
            };
            // end() already flushes
            let raw = match writer.end() {
                Ok(buf) => buf.into_inner().unwrap(),
                Err(e) => {
                    res = Err(From::from(e.0));
                    return Stream::Writing(e.1);
                }
            };
            Stream::Idle(raw)
        });
        res
    }
}

/// The `Protocol` implementation provides HTTP/1.1 messages.
pub struct Http11Protocol {
    connector: Connector,
}

impl Protocol for Http11Protocol {
    fn new_message(&self, host: &str, port: u16, scheme: &str) -> ::Result<Box<HttpMessage>> {
        let stream = try!(self.connector.connect(host, port, scheme)).into();

        Ok(Box::new(Http11Message::with_stream(stream)))
    }
}

impl Http11Protocol {
    /// Creates a new `Http11Protocol` instance that will use the given `NetworkConnector` for
    /// establishing HTTP connections.
    pub fn with_connector<C, S>(c: C) -> Http11Protocol
            where C: NetworkConnector<Stream=S> + Send + Sync + 'static,
                  S: NetworkStream + Send {
        Http11Protocol {
            connector: Connector(Box::new(ConnAdapter(c))),
        }
    }
}

struct ConnAdapter<C: NetworkConnector + Send + Sync>(C);

impl<C: NetworkConnector<Stream=S> + Send + Sync, S: NetworkStream + Send>
        NetworkConnector for ConnAdapter<C> {
    type Stream = Box<NetworkStream + Send>;
    #[inline]
    fn connect(&self, host: &str, port: u16, scheme: &str)
        -> ::Result<Box<NetworkStream + Send>> {
        Ok(try!(self.0.connect(host, port, scheme)).into())
    }
}

struct Connector(Box<NetworkConnector<Stream=Box<NetworkStream + Send>> + Send + Sync>);

impl NetworkConnector for Connector {
    type Stream = Box<NetworkStream + Send>;
    #[inline]
    fn connect(&self, host: &str, port: u16, scheme: &str)
        -> ::Result<Box<NetworkStream + Send>> {
        Ok(try!(self.0.connect(host, port, scheme)).into())
    }
}


/// Readers to handle different Transfer-Encodings.
///
/// If a message body does not include a Transfer-Encoding, it *should*
/// include a Content-Length header.
pub enum HttpReader<R> {
    /// A Reader used when a Content-Length header is passed with a positive integer.
    SizedReader(R, u64),
    /// A Reader used when Transfer-Encoding is `chunked`.
    ChunkedReader(R, Option<u64>),
    /// A Reader used for responses that don't indicate a length or chunked.
    ///
    /// Note: This should only used for `Response`s. It is illegal for a
    /// `Request` to be made with both `Content-Length` and
    /// `Transfer-Encoding: chunked` missing, as explained from the spec:
    ///
    /// > If a Transfer-Encoding header field is present in a response and
    /// > the chunked transfer coding is not the final encoding, the
    /// > message body length is determined by reading the connection until
    /// > it is closed by the server.  If a Transfer-Encoding header field
    /// > is present in a request and the chunked transfer coding is not
    /// > the final encoding, the message body length cannot be determined
    /// > reliably; the server MUST respond with the 400 (Bad Request)
    /// > status code and then close the connection.
    EofReader(R),
    /// A Reader used for messages that should never have a body.
    ///
    /// See https://tools.ietf.org/html/rfc7230#section-3.3.3
    EmptyReader(R),
}

impl<R: Read> HttpReader<R> {

    /// Unwraps this HttpReader and returns the underlying Reader.
    pub fn into_inner(self) -> R {
        match self {
            SizedReader(r, _) => r,
            ChunkedReader(r, _) => r,
            EofReader(r) => r,
            EmptyReader(r) => r,
        }
    }

    /// Gets a borrowed reference to the underlying Reader.
    pub fn get_ref(&self) -> &R {
        match *self {
            SizedReader(ref r, _) => r,
            ChunkedReader(ref r, _) => r,
            EofReader(ref r) => r,
            EmptyReader(ref r) => r,
        }
    }

    /// Gets a mutable reference to the underlying Reader.
    pub fn get_mut(&mut self) -> &mut R {
        match *self {
            SizedReader(ref mut r, _) => r,
            ChunkedReader(ref mut r, _) => r,
            EofReader(ref mut r) => r,
            EmptyReader(ref mut r) => r,
        }
    }
}

impl<R> fmt::Debug for HttpReader<R> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            SizedReader(_,rem) => write!(fmt, "SizedReader(remaining={:?})", rem),
            ChunkedReader(_, None) => write!(fmt, "ChunkedReader(chunk_remaining=unknown)"),
            ChunkedReader(_, Some(rem)) => write!(fmt, "ChunkedReader(chunk_remaining={:?})", rem),
            EofReader(_) => write!(fmt, "EofReader"),
            EmptyReader(_) => write!(fmt, "EmptyReader"),
        }
    }
}

impl<R: Read> Read for HttpReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        match *self {
            SizedReader(ref mut body, ref mut remaining) => {
                trace!("Sized read, remaining={:?}", remaining);
                if *remaining == 0 {
                    Ok(0)
                } else {
                    let to_read = min(*remaining as usize, buf.len());
                    let num = try!(body.read(&mut buf[..to_read])) as u64;
                    trace!("Sized read: {}", num);
                    if num > *remaining {
                        *remaining = 0;
                    } else if num == 0 {
                        return Err(io::Error::new(io::ErrorKind::Other, "early eof"));
                    } else {
                        *remaining -= num;
                    }
                    Ok(num as usize)
                }
            },
            ChunkedReader(ref mut body, ref mut opt_remaining) => {
                let mut rem = match *opt_remaining {
                    Some(ref rem) => *rem,
                    // None means we don't know the size of the next chunk
                    None => try!(read_chunk_size(body))
                };
                trace!("Chunked read, remaining={:?}", rem);

                if rem == 0 {
                    if opt_remaining.is_none() {
                        try!(eat(body, LINE_ENDING.as_bytes()));
                    }

                    *opt_remaining = Some(0);

                    // chunk of size 0 signals the end of the chunked stream
                    // if the 0 digit was missing from the stream, it would
                    // be an InvalidInput error instead.
                    trace!("end of chunked");

                    return Ok(0)
                }

                let to_read = min(rem as usize, buf.len());
                let count = try!(body.read(&mut buf[..to_read])) as u64;

                if count == 0 {
                    *opt_remaining = Some(0);
                    return Err(io::Error::new(io::ErrorKind::Other, "early eof"));
                }

                rem -= count;
                *opt_remaining = if rem > 0 {
                    Some(rem)
                } else {
                    try!(eat(body, LINE_ENDING.as_bytes()));
                    None
                };
                Ok(count as usize)
            },
            EofReader(ref mut body) => {
                let r = body.read(buf);
                trace!("eofread: {:?}", r);
                r
            },
            EmptyReader(_) => Ok(0)
        }
    }
}

fn eat<R: Read>(rdr: &mut R, bytes: &[u8]) -> io::Result<()> {
    let mut buf = [0];
    for &b in bytes.iter() {
        match try!(rdr.read(&mut buf)) {
            1 if buf[0] == b => (),
            _ => return Err(io::Error::new(io::ErrorKind::InvalidInput,
                                          "Invalid characters found")),
        }
    }
    Ok(())
}

/// Chunked chunks start with 1*HEXDIGIT, indicating the size of the chunk.
fn read_chunk_size<R: Read>(rdr: &mut R) -> io::Result<u64> {
    macro_rules! byte (
        ($rdr:ident) => ({
            let mut buf = [0];
            match try!($rdr.read(&mut buf)) {
                1 => buf[0],
                _ => return Err(io::Error::new(io::ErrorKind::InvalidInput,
                                                  "Invalid chunk size line")),

            }
        })
    );
    let mut size = 0u64;
    let radix = 16;
    let mut in_ext = false;
    let mut in_chunk_size = true;
    loop {
        match byte!(rdr) {
            b@b'0'...b'9' if in_chunk_size => {
                size *= radix;
                size += (b - b'0') as u64;
            },
            b@b'a'...b'f' if in_chunk_size => {
                size *= radix;
                size += (b + 10 - b'a') as u64;
            },
            b@b'A'...b'F' if in_chunk_size => {
                size *= radix;
                size += (b + 10 - b'A') as u64;
            },
            CR => {
                match byte!(rdr) {
                    LF => break,
                    _ => return Err(io::Error::new(io::ErrorKind::InvalidInput,
                                                  "Invalid chunk size line"))

                }
            },
            // If we weren't in the extension yet, the ";" signals its start
            b';' if !in_ext => {
                in_ext = true;
                in_chunk_size = false;
            },
            // "Linear white space" is ignored between the chunk size and the
            // extension separator token (";") due to the "implied *LWS rule".
            b'\t' | b' ' if !in_ext & !in_chunk_size => {},
            // LWS can follow the chunk size, but no more digits can come
            b'\t' | b' ' if in_chunk_size => in_chunk_size = false,
            // We allow any arbitrary octet once we are in the extension, since
            // they all get ignored anyway. According to the HTTP spec, valid
            // extensions would have a more strict syntax:
            //     (token ["=" (token | quoted-string)])
            // but we gain nothing by rejecting an otherwise valid chunk size.
            ext if in_ext => {
                todo!("chunk extension byte={}", ext);
            },
            // Finally, if we aren't in the extension and we're reading any
            // other octet, the chunk size line is invalid!
            _ => {
                return Err(io::Error::new(io::ErrorKind::InvalidInput,
                                         "Invalid chunk size line"));
            }
        }
    }
    trace!("chunk size={:?}", size);
    Ok(size)
}

fn should_have_response_body(method: &Method, status: u16) -> bool {
    trace!("should_have_response_body({:?}, {})", method, status);
    match (method, status) {
        (&Method::Head, _) |
        (_, 100...199) |
        (_, 204) |
        (_, 304) |
        (&Method::Connect, 200...299) => false,
        _ => true
    }
}

/// Writers to handle different Transfer-Encodings.
pub enum HttpWriter<W: Write> {
    /// A no-op Writer, used initially before Transfer-Encoding is determined.
    ThroughWriter(W),
    /// A Writer for when Transfer-Encoding includes `chunked`.
    ChunkedWriter(W),
    /// A Writer for when Content-Length is set.
    ///
    /// Enforces that the body is not longer than the Content-Length header.
    SizedWriter(W, u64),
    /// A writer that should not write any body.
    EmptyWriter(W),
}

impl<W: Write> HttpWriter<W> {
    /// Unwraps the HttpWriter and returns the underlying Writer.
    #[inline]
    pub fn into_inner(self) -> W {
        match self {
            ThroughWriter(w) => w,
            ChunkedWriter(w) => w,
            SizedWriter(w, _) => w,
            EmptyWriter(w) => w,
        }
    }

    /// Access the inner Writer.
    #[inline]
    pub fn get_ref(&self) -> &W {
        match *self {
            ThroughWriter(ref w) => w,
            ChunkedWriter(ref w) => w,
            SizedWriter(ref w, _) => w,
            EmptyWriter(ref w) => w,
        }
    }

    /// Access the inner Writer mutably.
    ///
    /// Warning: You should not write to this directly, as you can corrupt
    /// the state.
    #[inline]
    pub fn get_mut(&mut self) -> &mut W {
        match *self {
            ThroughWriter(ref mut w) => w,
            ChunkedWriter(ref mut w) => w,
            SizedWriter(ref mut w, _) => w,
            EmptyWriter(ref mut w) => w,
        }
    }

    /// Ends the HttpWriter, and returns the underlying Writer.
    ///
    /// A final `write_all()` is called with an empty message, and then flushed.
    /// The ChunkedWriter variant will use this to write the 0-sized last-chunk.
    #[inline]
    pub fn end(mut self) -> Result<W, EndError<W>> {
        fn inner<W: Write>(w: &mut W) -> io::Result<()> {
            try!(w.write(&[]));
            w.flush()
        }

        match inner(&mut self) {
            Ok(..) => Ok(self.into_inner()),
            Err(e) => Err(EndError(e, self))
        }
    }
}

#[derive(Debug)]
pub struct EndError<W: Write>(io::Error, HttpWriter<W>);

impl<W: Write> From<EndError<W>> for io::Error {
    fn from(e: EndError<W>) -> io::Error {
        e.0
    }
}

impl<W: Write> Write for HttpWriter<W> {
    #[inline]
    fn write(&mut self, msg: &[u8]) -> io::Result<usize> {
        match *self {
            ThroughWriter(ref mut w) => w.write(msg),
            ChunkedWriter(ref mut w) => {
                let chunk_size = msg.len();
                trace!("chunked write, size = {:?}", chunk_size);
                try!(write!(w, "{:X}{}", chunk_size, LINE_ENDING));
                try!(w.write_all(msg));
                try!(w.write_all(LINE_ENDING.as_bytes()));
                Ok(msg.len())
            },
            SizedWriter(ref mut w, ref mut remaining) => {
                let len = msg.len() as u64;
                if len > *remaining {
                    let len = *remaining;
                    *remaining = 0;
                    try!(w.write_all(&msg[..len as usize]));
                    Ok(len as usize)
                } else {
                    *remaining -= len;
                    try!(w.write_all(msg));
                    Ok(len as usize)
                }
            },
            EmptyWriter(..) => {
                if !msg.is_empty() {
                    error!("Cannot include a body with this kind of message");
                }
                Ok(0)
            }
        }
    }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        match *self {
            ThroughWriter(ref mut w) => w.flush(),
            ChunkedWriter(ref mut w) => w.flush(),
            SizedWriter(ref mut w, _) => w.flush(),
            EmptyWriter(ref mut w) => w.flush(),
        }
    }
}

impl<W: Write> fmt::Debug for HttpWriter<W> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ThroughWriter(_) => write!(fmt, "ThroughWriter"),
            ChunkedWriter(_) => write!(fmt, "ChunkedWriter"),
            SizedWriter(_, rem) => write!(fmt, "SizedWriter(remaining={:?})", rem),
            EmptyWriter(_) => write!(fmt, "EmptyWriter"),
        }
    }
}

const MAX_HEADERS: usize = 100;

/// Parses a request into an Incoming message head.
#[inline]
pub fn parse_request<R: Read>(buf: &mut BufReader<R>) -> ::Result<Incoming<(Method, RequestUri)>> {
    parse::<R, httparse::Request, (Method, RequestUri)>(buf)
}

/// Parses a response into an Incoming message head.
#[inline]
pub fn parse_response<R: Read>(buf: &mut BufReader<R>) -> ::Result<Incoming<RawStatus>> {
    parse::<R, httparse::Response, RawStatus>(buf)
}

fn parse<R: Read, T: TryParse<Subject=I>, I>(rdr: &mut BufReader<R>) -> ::Result<Incoming<I>> {
    loop {
        match try!(try_parse::<R, T, I>(rdr)) {
            httparse::Status::Complete((inc, len)) => {
                rdr.consume(len);
                return Ok(inc);
            },
            _partial => ()
        }
        match try!(rdr.read_into_buf()) {
            0 if rdr.get_buf().is_empty() => {
                return Err(Error::Io(io::Error::new(
                    io::ErrorKind::ConnectionAborted,
                    "Connection closed"
                )))
            },
            0 => return Err(Error::TooLarge),
            _ => ()
        }
    }
}

fn try_parse<R: Read, T: TryParse<Subject=I>, I>(rdr: &mut BufReader<R>) -> TryParseResult<I> {
    let mut headers = [httparse::EMPTY_HEADER; MAX_HEADERS];
    let buf = rdr.get_buf();
    if buf.len() == 0 {
        return Ok(httparse::Status::Partial);
    }
    trace!("try_parse({:?})", buf);
    <T as TryParse>::try_parse(&mut headers, buf)
}

#[doc(hidden)]
trait TryParse {
    type Subject;
    fn try_parse<'a>(headers: &'a mut [httparse::Header<'a>], buf: &'a [u8]) ->
        TryParseResult<Self::Subject>;
}

type TryParseResult<T> = Result<httparse::Status<(Incoming<T>, usize)>, Error>;

impl<'a> TryParse for httparse::Request<'a, 'a> {
    type Subject = (Method, RequestUri);

    fn try_parse<'b>(headers: &'b mut [httparse::Header<'b>], buf: &'b [u8]) ->
            TryParseResult<(Method, RequestUri)> {
        trace!("Request.try_parse([Header; {}], [u8; {}])", headers.len(), buf.len());
        let mut req = httparse::Request::new(headers);
        Ok(match try!(req.parse(buf)) {
            httparse::Status::Complete(len) => {
                trace!("Request.try_parse Complete({})", len);
                httparse::Status::Complete((Incoming {
                    version: if req.version.unwrap() == 1 { Http11 } else { Http10 },
                    subject: (
                        try!(req.method.unwrap().parse()),
                        try!(req.path.unwrap().parse())
                    ),
                    headers: try!(Headers::from_raw(req.headers))
                }, len))
            },
            httparse::Status::Partial => httparse::Status::Partial
        })
    }
}

impl<'a> TryParse for httparse::Response<'a, 'a> {
    type Subject = RawStatus;

    fn try_parse<'b>(headers: &'b mut [httparse::Header<'b>], buf: &'b [u8]) ->
            TryParseResult<RawStatus> {
        trace!("Response.try_parse([Header; {}], [u8; {}])", headers.len(), buf.len());
        let mut res = httparse::Response::new(headers);
        Ok(match try!(res.parse(buf)) {
            httparse::Status::Complete(len) => {
                trace!("Response.try_parse Complete({})", len);
                let code = res.code.unwrap();
                let reason = match StatusCode::from_u16(code).canonical_reason() {
                    Some(reason) if reason == res.reason.unwrap() => Cow::Borrowed(reason),
                    _ => Cow::Owned(res.reason.unwrap().to_owned())
                };
                httparse::Status::Complete((Incoming {
                    version: if res.version.unwrap() == 1 { Http11 } else { Http10 },
                    subject: RawStatus(code, reason),
                    headers: try!(Headers::from_raw(res.headers))
                }, len))
            },
            httparse::Status::Partial => httparse::Status::Partial
        })
    }
}

/// An Incoming Message head. Includes request/status line, and headers.
#[derive(Debug)]
pub struct Incoming<S> {
    /// HTTP version of the message.
    pub version: HttpVersion,
    /// Subject (request line or status line) of Incoming message.
    pub subject: S,
    /// Headers of the Incoming message.
    pub headers: Headers
}

/// The `\r` byte.
pub const CR: u8 = b'\r';
/// The `\n` byte.
pub const LF: u8 = b'\n';
/// The bytes `\r\n`.
pub const LINE_ENDING: &'static str = "\r\n";

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::io::{self, Read, Write};


    use buffer::BufReader;
    use mock::MockStream;
    use http::HttpMessage;

    use super::{read_chunk_size, parse_request, parse_response, Http11Message};

    #[test]
    fn test_write_chunked() {
        use std::str::from_utf8;
        let mut w = super::HttpWriter::ChunkedWriter(Vec::new());
        w.write_all(b"foo bar").unwrap();
        w.write_all(b"baz quux herp").unwrap();
        let buf = w.end().unwrap();
        let s = from_utf8(buf.as_ref()).unwrap();
        assert_eq!(s, "7\r\nfoo bar\r\nD\r\nbaz quux herp\r\n0\r\n\r\n");
    }

    #[test]
    fn test_write_sized() {
        use std::str::from_utf8;
        let mut w = super::HttpWriter::SizedWriter(Vec::new(), 8);
        w.write_all(b"foo bar").unwrap();
        assert_eq!(w.write(b"baz").unwrap(), 1);

        let buf = w.end().unwrap();
        let s = from_utf8(buf.as_ref()).unwrap();
        assert_eq!(s, "foo barb");
    }

    #[test]
    fn test_read_chunk_size() {
        fn read(s: &str, result: u64) {
            assert_eq!(read_chunk_size(&mut s.as_bytes()).unwrap(), result);
        }

        fn read_err(s: &str) {
            assert_eq!(read_chunk_size(&mut s.as_bytes()).unwrap_err().kind(),
                io::ErrorKind::InvalidInput);
        }

        read("1\r\n", 1);
        read("01\r\n", 1);
        read("0\r\n", 0);
        read("00\r\n", 0);
        read("A\r\n", 10);
        read("a\r\n", 10);
        read("Ff\r\n", 255);
        read("Ff   \r\n", 255);
        // Missing LF or CRLF
        read_err("F\rF");
        read_err("F");
        // Invalid hex digit
        read_err("X\r\n");
        read_err("1X\r\n");
        read_err("-\r\n");
        read_err("-1\r\n");
        // Acceptable (if not fully valid) extensions do not influence the size
        read("1;extension\r\n", 1);
        read("a;ext name=value\r\n", 10);
        read("1;extension;extension2\r\n", 1);
        read("1;;;  ;\r\n", 1);
        read("2; extension...\r\n", 2);
        read("3   ; extension=123\r\n", 3);
        read("3   ;\r\n", 3);
        read("3   ;   \r\n", 3);
        // Invalid extensions cause an error
        read_err("1 invalid extension\r\n");
        read_err("1 A\r\n");
        read_err("1;no CRLF");
    }

    #[test]
    fn test_read_sized_early_eof() {
        let mut r = super::HttpReader::SizedReader(MockStream::with_input(b"foo bar"), 10);
        let mut buf = [0u8; 10];
        assert_eq!(r.read(&mut buf).unwrap(), 7);
        let e = r.read(&mut buf).unwrap_err();
        assert_eq!(e.kind(), io::ErrorKind::Other);
        assert_eq!(e.description(), "early eof");
    }

    #[test]
    fn test_read_chunked_early_eof() {
        let mut r = super::HttpReader::ChunkedReader(MockStream::with_input(b"\
            9\r\n\
            foo bar\
        "), None);

        let mut buf = [0u8; 10];
        assert_eq!(r.read(&mut buf).unwrap(), 7);
        let e = r.read(&mut buf).unwrap_err();
        assert_eq!(e.kind(), io::ErrorKind::Other);
        assert_eq!(e.description(), "early eof");
    }

    #[test]
    fn test_read_sized_zero_len_buf() {
        let mut r = super::HttpReader::SizedReader(MockStream::with_input(b"foo bar"), 7);
        let mut buf = [0u8; 0];
        assert_eq!(r.read(&mut buf).unwrap(), 0);
    }

    #[test]
    fn test_read_chunked_zero_len_buf() {
        let mut r = super::HttpReader::ChunkedReader(MockStream::with_input(b"\
            7\r\n\
            foo bar\
            0\r\n\r\n\
        "), None);

        let mut buf = [0u8; 0];
        assert_eq!(r.read(&mut buf).unwrap(), 0);
    }

    #[test]
    fn test_read_chunked_fully_consumes() {
        let mut r = super::HttpReader::ChunkedReader(MockStream::with_input(b"0\r\n\r\n"), None);
        let mut buf = [0; 1];
        assert_eq!(r.read(&mut buf).unwrap(), 0);
        assert_eq!(r.read(&mut buf).unwrap(), 0);

        match r {
            super::HttpReader::ChunkedReader(mut r, _) => assert_eq!(r.read(&mut buf).unwrap(), 0),
            _ => unreachable!(),
        }
    }

    #[test]
    fn test_message_get_incoming_invalid_content_length() {
        let raw = MockStream::with_input(
            b"HTTP/1.1 200 OK\r\nContent-Length: asdf\r\n\r\n");
        let mut msg = Http11Message::with_stream(Box::new(raw));
        assert!(msg.get_incoming().is_err());
        assert!(msg.close_connection().is_ok());
    }

    #[test]
    fn test_parse_incoming() {
        let mut raw = MockStream::with_input(b"GET /echo HTTP/1.1\r\nHost: hyper.rs\r\n\r\n");
        let mut buf = BufReader::new(&mut raw);
        parse_request(&mut buf).unwrap();
    }

    #[test]
    fn test_parse_raw_status() {
        let mut raw = MockStream::with_input(b"HTTP/1.1 200 OK\r\n\r\n");
        let mut buf = BufReader::new(&mut raw);
        let res = parse_response(&mut buf).unwrap();

        assert_eq!(res.subject.1, "OK");

        let mut raw = MockStream::with_input(b"HTTP/1.1 200 Howdy\r\n\r\n");
        let mut buf = BufReader::new(&mut raw);
        let res = parse_response(&mut buf).unwrap();

        assert_eq!(res.subject.1, "Howdy");
    }


    #[test]
    fn test_parse_tcp_closed() {
        use std::io::ErrorKind;
        use error::Error;

        let mut empty = MockStream::new();
        let mut buf = BufReader::new(&mut empty);
        match parse_request(&mut buf) {
            Err(Error::Io(ref e)) if e.kind() == ErrorKind::ConnectionAborted => (),
            other => panic!("unexpected result: {:?}", other)
        }
    }

    #[cfg(feature = "nightly")]
    use test::Bencher;

    #[cfg(feature = "nightly")]
    #[bench]
    fn bench_parse_incoming(b: &mut Bencher) {
        let mut raw = MockStream::with_input(b"GET /echo HTTP/1.1\r\nHost: hyper.rs\r\n\r\n");
        let mut buf = BufReader::new(&mut raw);
        b.iter(|| {
            parse_request(&mut buf).unwrap();
            buf.get_mut().read.set_position(0);
        });
    }
}
