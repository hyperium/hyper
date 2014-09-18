//! Server Responses
//!
//! These are responses sent by a `hyper::Server` to clients, after
//! receiving a request.
use std::io::{BufferedWriter, IoResult};

use time::now_utc;

use header;
use header::common;
use http::{CR, LF, LINE_ENDING, HttpWriter, ThroughWriter, ChunkedWriter, SizedWriter};
use status;
use net::{NetworkStream, WriteStatus, Fresh, Streaming};
use version;

/// The outgoing half for a Tcp connection, created by a `Server` and given to a `Handler`.
pub struct Response<W: WriteStatus> {
    /// The HTTP version of this response.
    pub version: version::HttpVersion,
    // Stream the Response is writing to, not accessible through UnwrittenResponse
    body: HttpWriter<BufferedWriter<Box<NetworkStream + Send>>>,
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

    /// Construct a Response from its constituent parts.
    pub fn construct(version: version::HttpVersion,
                     body: HttpWriter<BufferedWriter<Box<NetworkStream + Send>>>,
                     status: status::StatusCode,
                     headers: header::Headers) -> Response<Fresh> {
        Response {
            status: status,
            version: version,
            body: body,
            headers: headers
        }
    }
}

impl Response<Fresh> {
    /// Creates a new Response that can be used to write to a network stream.
    pub fn new<S: NetworkStream>(stream: S) -> Response<Fresh> {
        Response {
            status: status::Ok,
            version: version::Http11,
            headers: header::Headers::new(),
            body: ThroughWriter(BufferedWriter::new(stream.abstract()))
        }
    }

    /// Consume this Response<Fresh>, writing the Headers and Status and creating a Response<Streaming>
    pub fn start(mut self) -> IoResult<Response<Streaming>> {
        debug!("writing head: {} {}", self.version, self.status);
        try!(write!(self.body, "{} {}{}{}", self.version, self.status, CR as char, LF as char));

        if !self.headers.has::<common::Date>() {
            self.headers.set(common::Date(now_utc()));
        }


        let mut chunked = true;
        let mut len = 0;

        match self.headers.get_ref::<common::ContentLength>() {
            Some(cl) => {
                chunked = false;
                len = cl.len();
            },
            None => ()
        };

        // cant do in match above, thanks borrowck
        if chunked {
            //TODO: use CollectionViews (when implemented) to prevent double hash/lookup
            let encodings = match self.headers.get::<common::TransferEncoding>() {
                Some(common::TransferEncoding(mut encodings)) => {
                    //TODO: check if chunked is already in encodings. use HashSet?
                    encodings.push(common::transfer_encoding::Chunked);
                    encodings
                },
                None => vec![common::transfer_encoding::Chunked]
            };
            self.headers.set(common::TransferEncoding(encodings));
        }

        for (name, header) in self.headers.iter() {
            debug!("header {}: {}", name, header);
            try!(write!(self.body, "{}: {}", name, header));
            try!(self.body.write(LINE_ENDING));
        }

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

    /// Deconstruct this Response into its constituent parts.
    pub fn deconstruct(self) -> (version::HttpVersion, HttpWriter<BufferedWriter<Box<NetworkStream + Send>>>,
                                 status::StatusCode, header::Headers) {
        (self.version, self.body, self.status, self.headers)
    }
}

impl Response<Streaming> {
    /// Flushes all writing of a response to the client.
    pub fn end(self) -> IoResult<()> {
        debug!("ending");
        try!(self.body.end());
        Ok(())
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

