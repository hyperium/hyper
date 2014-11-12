//! Server Requests
//!
//! These are requests that a `hyper::Server` receives, and include its method,
//! target URI, headers, and message body.
use std::io::IoResult;
use std::io::net::ip::SocketAddr;

use {HttpResult};
use version::{HttpVersion};
use method;
use header::Headers;
use header::common::ContentLength;
use http::{read_request_line};
use http::HttpReader;
use http::HttpReader::{SizedReader, ChunkedReader};
use uri::RequestUri;

pub type InternalReader<'a> = &'a mut Reader + 'a;

/// A request bundles several parts of an incoming `NetworkStream`, given to a `Handler`.
pub struct Request<'a> {
    /// The IP address of the remote connection.
    pub remote_addr: SocketAddr,
    /// The `Method`, such as `Get`, `Post`, etc.
    pub method: method::Method,
    /// The headers of the incoming request.
    pub headers: Headers,
    /// The target request-uri for this request.
    pub uri: RequestUri,
    /// The version of HTTP for this request.
    pub version: HttpVersion,
    body: HttpReader<InternalReader<'a>>
}


impl<'a> Request<'a> {

    /// Create a new Request, reading the StartLine and Headers so they are
    /// immediately useful.
    pub fn new(mut stream: InternalReader<'a>, addr: SocketAddr) -> HttpResult<Request<'a>> {
        let (method, uri, version) = try!(read_request_line(&mut stream));
        debug!("Request Line: {} {} {}", method, uri, version);
        let headers = try!(Headers::from_raw(&mut stream));
        debug!("Headers: [\n{}]", headers);


        let body = if headers.has::<ContentLength>() {
            match headers.get::<ContentLength>() {
                Some(&ContentLength(len)) => SizedReader(stream, len),
                None => unreachable!()
            }
        } else {
            todo!("check for Transfer-Encoding: chunked");
            ChunkedReader(stream, None)
        };

        Ok(Request {
            remote_addr: addr,
            method: method,
            uri: uri,
            headers: headers,
            version: version,
            body: body
        })
    }
}

impl<'a> Reader for Request<'a> {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<uint> {
        self.body.read(buf)
    }
}

