//! Client Requests
use std::io::{BufferedWriter, IoResult};

use url::Url;

use method::{mod, Get, Post, Delete, Put, Patch, Head, Options};
use header::Headers;
use header::common::{mod, Host};
use net::{NetworkStream, HttpStream, Fresh, Streaming};
use http::{HttpWriter, ThroughWriter, ChunkedWriter, SizedWriter, LINE_ENDING};
use version;
use {HttpResult, HttpUriError};
use client::Response;


/// A client request to a remote server.
pub struct Request<W> {
    /// The target URI for this request.
    pub url: Url,

    /// The HTTP version of this request.
    pub version: version::HttpVersion,

    body: HttpWriter<BufferedWriter<Box<NetworkStream + Send>>>,
    headers: Headers,
    method: method::Method,
}

impl<W> Request<W> {
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

        let stream: S = try_io!(NetworkStream::connect(host.as_slice(), port, url.scheme.as_slice()));
        let stream = ThroughWriter(BufferedWriter::new(stream.dynamic()));
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

        let mut chunked = true;
        let mut len = 0;

        match self.headers.get::<common::ContentLength>() {
            Some(cl) => {
                chunked = false;
                len = cl.len();
            },
            None => ()
        };

        // cant do in match above, thanks borrowck
        if chunked {
            let encodings = match self.headers.get_mut::<common::TransferEncoding>() {
                Some(&common::TransferEncoding(ref mut encodings)) => {
                    //TODO: check if chunked is already in encodings. use HashSet?
                    encodings.push(common::transfer_encoding::Chunked);
                    false
                },
                None => true
            };

            if encodings {
                self.headers.set::<common::TransferEncoding>(
                    common::TransferEncoding(vec![common::transfer_encoding::Chunked]))
            }
        }

        for (name, header) in self.headers.iter() {
            try_io!(write!(self.body, "{}: {}", name, header));
            try_io!(self.body.write(LINE_ENDING));
        }

        try_io!(self.body.write(LINE_ENDING));

        let stream = if chunked {
            ChunkedWriter(self.body.unwrap())
        } else {
            SizedWriter(self.body.unwrap(), len)
        };

        Ok(Request {
            method: self.method,
            headers: self.headers,
            url: self.url,
            version: self.version,
            body: stream
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
    pub fn send(self) -> HttpResult<Response> {
        let raw = try_io!(self.body.end()).unwrap();
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

