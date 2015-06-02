//! Adapts the HTTP/1.1 implementation into the `HttpMessage` API.
use std::io::{self, Write, BufWriter, Read};
use std::net::Shutdown;

use method::{Method};
use header::{ContentLength, TransferEncoding};
use header::Encoding::Chunked;

use net::{NetworkConnector, NetworkStream, ContextVerifier};
use http::{HttpWriter, LINE_ENDING};
use http::HttpReader::{SizedReader, ChunkedReader, EofReader};
use http::HttpWriter::{ChunkedWriter, SizedWriter, EmptyWriter};
use buffer::BufReader;
use http::{self, HttpReader};

use message::{
    Protocol,
    HttpMessage,
    RequestHead,
    ResponseHead,
};
use header;
use version;

/// An implementation of the `HttpMessage` trait for HTTP/1.1.
#[derive(Debug)]
pub struct Http11Message {
    stream: Option<Box<NetworkStream + Send>>,
    writer: Option<HttpWriter<BufWriter<Box<NetworkStream + Send>>>>,
    reader: Option<HttpReader<BufReader<Box<NetworkStream + Send>>>>,
}

impl Write for Http11Message {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self.writer {
            None => Err(io::Error::new(io::ErrorKind::Other,
                                          "Not in a writable state")),
            Some(ref mut writer) => writer.write(buf),
        }
    }
    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        match self.writer {
            None => Err(io::Error::new(io::ErrorKind::Other,
                                          "Not in a writable state")),
            Some(ref mut writer) => writer.flush(),
        }
    }
}

impl Read for Http11Message {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self.reader {
            None => Err(io::Error::new(io::ErrorKind::Other,
                                          "Not in a readable state")),
            Some(ref mut reader) => reader.read(buf),
        }
    }
}

impl HttpMessage for Http11Message {
    fn set_outgoing(&mut self, mut head: RequestHead) -> ::Result<RequestHead> {
        if self.stream.is_none() {
            return Err(From::from(io::Error::new(
                        io::ErrorKind::Other,
                        "Message not idle, cannot start new outgoing")));
        }
        let mut stream = BufWriter::new(self.stream.take().unwrap());

        let mut uri = head.url.serialize_path().unwrap();
        if let Some(ref q) = head.url.query {
            uri.push('?');
            uri.push_str(&q[..]);
        }

        let version = version::HttpVersion::Http11;
        debug!("request line: {:?} {:?} {:?}", head.method, uri, version);
        try!(write!(&mut stream, "{} {} {}{}",
                    head.method, uri, version, LINE_ENDING));

        let stream = match head.method {
            Method::Get | Method::Head => {
                debug!("headers={:?}", head.headers);
                try!(write!(&mut stream, "{}{}", head.headers, LINE_ENDING));
                EmptyWriter(stream)
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
                        Some(&mut header::TransferEncoding(ref mut encodings)) => {
                            //TODO: check if chunked is already in encodings. use HashSet?
                            encodings.push(header::Encoding::Chunked);
                            false
                        },
                        None => true
                    };

                    if encodings {
                        head.headers.set::<header::TransferEncoding>(
                            header::TransferEncoding(vec![header::Encoding::Chunked]))
                    }
                }

                debug!("headers={:?}", head.headers);
                try!(write!(&mut stream, "{}{}", head.headers, LINE_ENDING));

                if chunked {
                    ChunkedWriter(stream)
                } else {
                    SizedWriter(stream, len)
                }
            }
        };

        self.writer = Some(stream);

        Ok(head)
    }

    fn get_incoming(&mut self) -> ::Result<ResponseHead> {
        try!(self.flush_outgoing());
        if self.stream.is_none() {
            // The message was already in the reading state...
            // TODO Decide what happens in case we try to get a new incoming at that point
            return Err(From::from(
                    io::Error::new(io::ErrorKind::Other,
                    "Read already in progress")));
        }

        let stream = self.stream.take().unwrap();
        let mut stream = BufReader::new(stream);

        let head = try!(http::parse_response(&mut stream));
        let raw_status = head.subject;
        let headers = head.headers;

        let body = if headers.has::<TransferEncoding>() {
            match headers.get::<TransferEncoding>() {
                Some(&TransferEncoding(ref codings)) => {
                    if codings.len() > 1 {
                        trace!("TODO: #2 handle other codings: {:?}", codings);
                    };

                    if codings.contains(&Chunked) {
                        ChunkedReader(stream, None)
                    } else {
                        trace!("not chuncked. read till eof");
                        EofReader(stream)
                    }
                }
                None => unreachable!()
            }
        } else if headers.has::<ContentLength>() {
            match headers.get::<ContentLength>() {
                Some(&ContentLength(len)) => SizedReader(stream, len),
                None => unreachable!()
            }
        } else {
            trace!("neither Transfer-Encoding nor Content-Length");
            EofReader(stream)
        };

        self.reader = Some(body);

        Ok(ResponseHead {
            headers: headers,
            raw_status: raw_status,
            version: head.version,
        })
    }

    fn close_connection(&mut self) -> ::Result<()> {
        try!(self.get_mut().close(Shutdown::Both));
        Ok(())
    }
}

impl Http11Message {
    /// Consumes the `Http11Message` and returns the underlying `NetworkStream`.
    pub fn into_inner(mut self) -> Box<NetworkStream + Send> {
        if self.stream.is_some() {
            self.stream.take().unwrap()
        } else if self.writer.is_some() {
            self.writer.take().unwrap().into_inner().into_inner().unwrap()
        } else if self.reader.is_some() {
            self.reader.take().unwrap().into_inner().into_inner()
        } else {
            panic!("Http11Message lost its underlying stream somehow");
        }
    }

    /// Gets a mutable reference to the underlying `NetworkStream`, regardless of the state of the
    /// `Http11Message`.
    pub fn get_mut(&mut self) -> &mut Box<NetworkStream + Send> {
        if self.stream.is_some() {
            self.stream.as_mut().unwrap()
        } else if self.writer.is_some() {
            self.writer.as_mut().unwrap().get_mut().get_mut()
        } else if self.reader.is_some() {
            self.reader.as_mut().unwrap().get_mut().get_mut()
        } else {
            panic!("Http11Message lost its underlying stream somehow");
        }
    }

    /// Creates a new `Http11Message` that will use the given `NetworkStream` for communicating to
    /// the peer.
    pub fn with_stream(stream: Box<NetworkStream + Send>) -> Http11Message {
        Http11Message {
            stream: Some(stream),
            writer: None,
            reader: None,
        }
    }

    /// Flushes the current outgoing content and moves the stream into the `stream` property.
    ///
    /// TODO It might be sensible to lift this up to the `HttpMessage` trait itself...
    pub fn flush_outgoing(&mut self) -> ::Result<()> {
        match self.writer {
            None => return Ok(()),
            Some(_) => {},
        };

        let writer = self.writer.take().unwrap();
        let raw = try!(writer.end()).into_inner().unwrap(); // end() already flushes
        self.stream = Some(raw);

        Ok(())
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

    #[inline]
    fn set_ssl_verifier(&mut self, verifier: ContextVerifier) {
        self.connector.set_ssl_verifier(verifier);
    }
}

impl Http11Protocol {
    /// Creates a new `Http11Protocol` instance that will use the given `NetworkConnector` for
    /// establishing HTTP connections.
    pub fn with_connector<C, S>(c: C) -> Http11Protocol
            where C: NetworkConnector<Stream=S> + Send + 'static,
                  S: NetworkStream + Send {
        Http11Protocol {
            connector: Connector(Box::new(ConnAdapter(c))),
        }
    }
}

struct ConnAdapter<C: NetworkConnector + Send>(C);

impl<C: NetworkConnector<Stream=S> + Send, S: NetworkStream + Send> NetworkConnector for ConnAdapter<C> {
    type Stream = Box<NetworkStream + Send>;
    #[inline]
    fn connect(&self, host: &str, port: u16, scheme: &str)
        -> ::Result<Box<NetworkStream + Send>> {
        Ok(try!(self.0.connect(host, port, scheme)).into())
    }
    #[inline]
    fn set_ssl_verifier(&mut self, verifier: ContextVerifier) {
        self.0.set_ssl_verifier(verifier);
    }
}

struct Connector(Box<NetworkConnector<Stream=Box<NetworkStream + Send>> + Send>);

impl NetworkConnector for Connector {
    type Stream = Box<NetworkStream + Send>;
    #[inline]
    fn connect(&self, host: &str, port: u16, scheme: &str)
        -> ::Result<Box<NetworkStream + Send>> {
        Ok(try!(self.0.connect(host, port, scheme)).into())
    }
    #[inline]
    fn set_ssl_verifier(&mut self, verifier: ContextVerifier) {
        self.0.set_ssl_verifier(verifier);
    }
}
