//! Client Requests
use std::io::{BufferedWriter, IoResult};

use url::Url;

use method;
use header::Headers;
use header::common::Host;
use net::{NetworkStream, HttpStream, WriteStatus, Fresh, Streaming};
use http::LINE_ENDING;
use version;
use {HttpResult, HttpUriError};
use super::{Response};


/// A client request to a remote server.
pub struct Request<W: WriteStatus> {
    version: version::HttpVersion,
    method: method::Method,
    headers: Headers,
    url: Url,
    body: BufferedWriter<Box<NetworkStream + Send>>,
}

impl<W: WriteStatus> Request<W> {
    /// The method of this request..
    #[inline]
    pub fn method(&self) -> &method::Method { &self.method }

    /// The headers of this request.
    pub fn headers(&self) -> &Headers { &self.headers }

    /// The version of this request.
    #[inline]
    pub fn version(&self) -> version::HttpVersion { self.version }

}

impl Request<Fresh> {

    /// Create a new client request.
    pub fn new(method: method::Method, url: Url) -> HttpResult<Request<Fresh>> {
        debug!("{} {}", method, url);
        let host = match url.serialize_host() {
            Some(host) => host,
            None => return Err(HttpUriError)
        };
        debug!("host={}", host);
        let port = match url.port_or_default() {
            Some(port) => port,
            None => return Err(HttpUriError)
        };
        debug!("port={}", port);

        let stream: HttpStream = try_io!(NetworkStream::connect(host.as_slice(), port));
        let stream = BufferedWriter::new(stream.abstract());
        let mut headers = Headers::new();
        headers.set(Host(host));
        Ok(Request {
            method: method,
            headers: headers,
            url: url,
            version: version::Http11,
            body: stream
        })
    }

    /// Get a mutable reference to the Headers.
    #[inline]
    pub fn headers_mut(&mut self) -> &mut Headers { &mut self.headers }

    /// Get a mutable reference to the Method.
    #[inline]
    pub fn method_mut(&mut self) -> &mut method::Method { &mut self.method }

    /// Get a mutable reference to the HttpVersion.
    #[inline]
    pub fn version_mut(&mut self) -> &mut version::HttpVersion { &mut self.version }

    fn write_head(&mut self) -> IoResult<()> {
        let uri = self.url.serialize_path().unwrap();
        debug!("writing head: {} {} {}", self.method, uri, self.version);
        try!(write!(self.body, "{} {} {}", self.method, uri, self.version))
        try!(self.body.write(LINE_ENDING));

        debug!("{}", self.headers);

        for (name, header) in self.headers.iter() {
            try!(write!(self.body, "{}: {}", name, header));
            try!(self.body.write(LINE_ENDING));
        }

        self.body.write(LINE_ENDING)
    }

    /// Writes the StatusCode and Headers to the underlying stream, and returns
    /// a Streaming Request to write an optional body.
    ///
    /// Consumes the Request<Fresh>.
    pub fn start(mut self) -> IoResult<Request<Streaming>> {
        try!(self.write_head());

        // "copy" to change the phantom type
        Ok(Request {
            version: self.version,
            body: self.body,
            method: self.method,
            url: self.url,
            headers: self.headers
        })
    }
}

impl Request<Streaming> {
    /// Completes writing the request, and returns a response to read from.
    ///
    /// Consumes the Request.
    pub fn send(mut self) -> HttpResult<Response> {
        try_io!(self.flush());
        let raw = self.body.unwrap();
        Response::new(raw)
    }
}



impl Writer for Request<Streaming> {
    #[inline]
    fn write(&mut self, msg: &[u8]) -> IoResult<()> {
        self.body.write(msg)
    }

    #[inline]
    fn flush(&mut self) -> IoResult<()> {
        self.body.flush()
    }
}

