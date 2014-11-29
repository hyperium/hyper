//! Server Responses
//!
//! These are responses sent by a `hyper::Server` to clients, after
//! receiving a request.
use std::io::IoResult;

use time::now_utc;

use header;
use header::common;
use http::{CR, LF, LINE_ENDING, HttpWriter};
use http::HttpWriter::{ThroughWriter, ChunkedWriter, SizedWriter};
use status;
use net::{Fresh, Streaming};
use version;

pub type InternalWriter<'a> = &'a mut (Writer + 'a);

/// The outgoing half for a Tcp connection, created by a `Server` and given to a `Handler`.
pub struct Response<'a, W = Fresh> {
    /// The HTTP version of this response.
    pub version: version::HttpVersion,
    // Stream the Response is writing to, not accessible through UnwrittenResponse
    body: HttpWriter<InternalWriter<'a>>,
    // The status code for the request.
    status: status::StatusCode,
    // The outgoing headers on this response.
    headers: header::Headers
}

impl<'a, W> Response<'a, W> {
    /// The status of this response.
    #[inline]
    pub fn status(&self) -> status::StatusCode { self.status }

    /// The headers of this response.
    pub fn headers(&self) -> &header::Headers { &self.headers }

    /// Construct a Response from its constituent parts.
    pub fn construct(version: version::HttpVersion,
                     body: HttpWriter<InternalWriter<'a>>,
                     status: status::StatusCode,
                     headers: header::Headers) -> Response<'a, Fresh> {
        Response {
            status: status,
            version: version,
            body: body,
            headers: headers
        }
    }

    /// Deconstruct this Response into its constituent parts.
    pub fn deconstruct(self) -> (version::HttpVersion, HttpWriter<InternalWriter<'a>>,
                                 status::StatusCode, header::Headers) {
        (self.version, self.body, self.status, self.headers)
    }
}

impl<'a> Response<'a, Fresh> {
    /// Creates a new Response that can be used to write to a network stream.
    pub fn new(stream: InternalWriter<'a>) -> Response<'a, Fresh> {
        Response {
            status: status::StatusCode::Ok,
            version: version::HttpVersion::Http11,
            headers: header::Headers::new(),
            body: ThroughWriter(stream)
        }
    }

    /// Consume this Response<Fresh>, writing the Headers and Status and creating a Response<Streaming>
    pub fn start(mut self) -> IoResult<Response<'a, Streaming>> {
        debug!("writing head: {} {}", self.version, self.status);
        try!(write!(&mut self.body, "{} {}{}{}", self.version, self.status, CR as char, LF as char));

        if !self.headers.has::<common::Date>() {
            self.headers.set(common::Date(now_utc()));
        }


        let mut chunked = true;
        let mut len = 0;

        match self.headers.get::<common::ContentLength>() {
            Some(cl) => {
                chunked = false;
                len = **cl;
            },
            None => ()
        };

        // cant do in match above, thanks borrowck
        if chunked {
            let encodings = match self.headers.get_mut::<common::TransferEncoding>() {
                Some(&common::TransferEncoding(ref mut encodings)) => {
                    //TODO: check if chunked is already in encodings. use HashSet?
                    encodings.push(common::transfer_encoding::Encoding::Chunked);
                    false
                },
                None => true
            };

            if encodings {
                self.headers.set::<common::TransferEncoding>(
                    common::TransferEncoding(vec![common::transfer_encoding::Encoding::Chunked]))
            }
        }


        debug!("headers [\n{}]", self.headers);
        try!(write!(&mut self.body, "{}", self.headers));

        try!(self.body.write(LINE_ENDING));

        let stream = if chunked {
            ChunkedWriter(self.body.unwrap())
        } else {
            SizedWriter(self.body.unwrap(), len)
        };

        // "copy" to change the phantom type
        Ok(Response {
            version: self.version,
            body: stream,
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

impl<'a> Response<'a, Streaming> {
    /// Flushes all writing of a response to the client.
    pub fn end(self) -> IoResult<()> {
        debug!("ending");
        try!(self.body.end());
        Ok(())
    }
}

impl<'a> Writer for Response<'a, Streaming> {
    fn write(&mut self, msg: &[u8]) -> IoResult<()> {
        debug!("write {} bytes", msg.len());
        self.body.write(msg)
    }

    fn flush(&mut self) -> IoResult<()> {
        self.body.flush()
    }
}

