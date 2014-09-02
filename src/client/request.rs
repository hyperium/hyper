//! # Client Requests
use std::io::net::tcp::TcpStream;
use std::io::IoResult;

use url::Url;

use method;
use header::{Headers, Host};
use rfc7230::LINE_ENDING;
use version;
use {HttpResult, HttpUriError};
use super::{Response};


/// A client request to a remote server.
pub struct Request {
    /// The method of this request.
    pub method: method::Method,
    /// The headers that will be sent with this request.
    pub headers: Headers,
    /// The target URI for this request.
    pub url: Url,
    /// The HTTP version of this request.
    pub version: version::HttpVersion,
    headers_written: bool,
    body: TcpStream,
}

impl Request {

    /// Create a new client request.
    pub fn new(method: method::Method, url: Url) -> HttpResult<Request> {
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

        let stream = try_io!(TcpStream::connect(host.as_slice(), port));
        let mut headers = Headers::new();
        headers.set(Host(host));
        Ok(Request {
            method: method,
            headers: headers,
            url: url,
            version: version::Http11,
            headers_written: false,
            body: stream
        })
    }

    fn write_head(&mut self) -> IoResult<()> {
        if self.headers_written {
            debug!("headers previsouly written, nooping");
            return Ok(());
        }
        self.headers_written = true;

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

    /// Completes writing the request, and returns a response to read from.
    ///
    /// Consumes the Request.
    pub fn send(mut self) -> HttpResult<Response> {
        try_io!(self.flush());
        try_io!(self.body.close_write());
        Response::new(self.body)
    }
}


impl Writer for Request {
    fn write(&mut self, msg: &[u8]) -> IoResult<()> {
        if !self.headers_written {
            try!(self.write_head());
        }
        self.body.write(msg)
    }

    fn flush(&mut self) -> IoResult<()> {
        if !self.headers_written {
            try!(self.write_head());
        }
        self.body.flush()
    }
}

