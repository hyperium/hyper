//! Server Responses
//!
//! These are responses sent by a `hyper::Server` to clients, after
//! receiving a request.
use std::io::{BufferedWriter, IoResult};
use std::io::net::tcp::TcpStream;

use time::now_utc;

use header;
use status;
use version;
use rfc7230::{CR, LF, LINE_ENDING};

/// Phantom type indicating Headers and StatusCode have not been written.
pub struct Fresh;

/// Phantom type indicating Headers and StatusCode have been written.
pub struct Streaming;

/// The status of a Response, indicating if the headers and status have been written.
pub trait WriteStatus {}

impl WriteStatus for Streaming {}
impl WriteStatus for Fresh {}

/// The outgoing half for a Tcp connection, created by a `Server` and given to a `Handler`.
pub struct Response<W: WriteStatus> {
    /// The HTTP version of this response.
    pub version: version::HttpVersion,
    // Stream the Response is writing to, not accessible through UnwrittenResponse
    body: BufferedWriter<TcpStream>, // TODO: use a HttpWriter from rfc7230
    // The status code for the request.
    status: status::StatusCode,
    // The outgoing headers on this response.
    headers: header::Headers
}

impl<W: WriteStatus> Response<W> {
    /// The status of this response.
    #[inline]
    pub fn status(&self) -> status::StatusCode { self.status }

    /// The headers of this response.
    pub fn headers(&self) -> &header::Headers { &self.headers }
}

impl Response<Fresh> {
    /// Creates a new Response that can be used to write to a network stream.
    pub fn new(tcp: TcpStream) -> Response<Fresh> {
        Response {
            status: status::Ok,
            version: version::Http11,
            headers: header::Headers::new(),
            body: BufferedWriter::new(tcp)
        }
    }

    /// Consume this Response<Fresh>, writing the Headers and Status and creating a Response<Streaming>
    pub fn start(mut self) -> IoResult<Response<Streaming>> {
        debug!("writing head: {} {}", self.version, self.status);
        try!(write!(self.body, "{} {}{}{}", self.version, self.status, CR as char, LF as char));

        if !self.headers.has::<header::Date>() {
            self.headers.set(header::Date(now_utc()));
        }

        for (name, header) in self.headers.iter() {
            debug!("headers {}: {}", name, header);
            try!(write!(self.body, "{}: {}", name, header));
            try!(self.body.write(LINE_ENDING));
        }

        try!(self.body.write(LINE_ENDING));

        // "copy" to change the phantom type
        Ok(Response {
            version: self.version,
            body: self.body,
            status: self.status,
            headers: self.headers
        })
    }

    /// Get a mutable reference to the status.
    #[inline]
    pub fn status_mut(&mut self) -> &mut status::StatusCode { &mut self.status }

    /// Get a mutable reference to the Headers.
    pub fn headers_mut(&mut self) -> &mut header::Headers { &mut self.headers }
}

impl Response<Streaming> {
    /// Flushes all writing of a response to the client.
    pub fn end(mut self) -> IoResult<()> {
        debug!("ending");
        self.flush()
    }
}

impl Writer for Response<Streaming> {
    fn write(&mut self, msg: &[u8]) -> IoResult<()> {
        debug!("write {:u} bytes", msg.len());
        self.body.write(msg)
    }

    fn flush(&mut self) -> IoResult<()> {
        self.body.flush()
    }
}

