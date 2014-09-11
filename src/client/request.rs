//! Client Requests
use std::io::{BufferedWriter, IoResult};

use url::Url;

use method::{mod, Get, Post, Delete, Put, Patch, Head, Options};
use header::Headers;
use header::common::Host;
use net::{NetworkStream, HttpStream, WriteStatus, Fresh, Streaming};
use http::LINE_ENDING;
use version;
use {HttpResult, HttpUriError};
use client::Response;

/// A client request to a remote server.
pub struct Request<W: WriteStatus> {
    /// The target URI for this request.
    pub url: Url,

    /// The HTTP version of this request.
    pub version: version::HttpVersion,

    body: BufferedWriter<Box<NetworkStream + Send>>,
    headers: Headers,
    method: method::Method,
}

impl<W: WriteStatus> Request<W> {
    /// Read the Request headers.
    #[inline]
    pub fn headers(&self) -> &Headers { &self.headers }

    /// Read the Request method.
    #[inline]
    pub fn method(&self) -> method::Method { self.method.clone() }
}

impl Request<Fresh> {
    /// Create a new client request.
    pub fn new(method: method::Method, url: Url) -> HttpResult<Request<Fresh>> {
        Request::with_stream::<HttpStream>(method, url)
    }

    /// Create a new client request with a specific underlying NetworkStream.
    pub fn with_stream<S: NetworkStream>(method: method::Method, url: Url) -> HttpResult<Request<Fresh>> {
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

        let stream: S = try_io!(NetworkStream::connect(host.as_slice(), port));
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

    /// Create a new GET request.
    #[inline]
    pub fn get(url: Url) -> HttpResult<Request<Fresh>> { Request::new(Get, url) }

    /// Create a new POST request.
    #[inline]
    pub fn post(url: Url) -> HttpResult<Request<Fresh>> { Request::new(Post, url) }

    /// Create a new DELETE request.
    #[inline]
    pub fn delete(url: Url) -> HttpResult<Request<Fresh>> { Request::new(Delete, url) }

    /// Create a new PUT request.
    #[inline]
    pub fn put(url: Url) -> HttpResult<Request<Fresh>> { Request::new(Put, url) }

    /// Create a new PATCH request.
    #[inline]
    pub fn patch(url: Url) -> HttpResult<Request<Fresh>> { Request::new(Patch, url) }

    /// Create a new HEAD request.
    #[inline]
    pub fn head(url: Url) -> HttpResult<Request<Fresh>> { Request::new(Head, url) }

    /// Create a new OPTIONS request.
    #[inline]
    pub fn options(url: Url) -> HttpResult<Request<Fresh>> { Request::new(Options, url) }

    /// Consume a Fresh Request, writing the headers and method,
    /// returning a Streaming Request.
    pub fn start(mut self) -> HttpResult<Request<Streaming>> {
        let uri = self.url.serialize_path().unwrap();
        debug!("writing head: {} {} {}", self.method, uri, self.version);
        try_io!(write!(self.body, "{} {} {}", self.method, uri, self.version))
        try_io!(self.body.write(LINE_ENDING));

        debug!("{}", self.headers);

        for (name, header) in self.headers.iter() {
            try_io!(write!(self.body, "{}: {}", name, header));
            try_io!(self.body.write(LINE_ENDING));
        }

        try_io!(self.body.write(LINE_ENDING));

        Ok(Request {
            method: self.method,
            headers: self.headers,
            url: self.url,
            version: self.version,
            body: self.body
        })
    }

    /// Get a mutable reference to the Request headers.
    #[inline]
    pub fn headers_mut(&mut self) -> &mut Headers { &mut self.headers }
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
    fn write(&mut self, msg: &[u8]) -> IoResult<()> {
        self.body.write(msg)
    }

    fn flush(&mut self) -> IoResult<()> {
        self.body.flush()
    }
}

