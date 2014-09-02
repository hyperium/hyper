//! Server Requests
//!
//! These are requests that a `hyper::Server` receives, and include its method,
//! target URI, headers, and message body.
use std::io::{Reader, BufferedReader, IoResult};
use std::io::net::ip::SocketAddr;
use std::io::net::tcp::TcpStream;

use {HttpResult};
use version::{HttpVersion};
use method;
use header::{Headers, ContentLength};
use rfc7230::{read_request_line};
use rfc7230::{HttpReader, SizedReader, ChunkedReader};
use uri::RequestUri;

/// A request bundles several parts of an incoming TCP stream, given to a `Handler`.
pub struct Request {
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
    body: HttpReader<BufferedReader<TcpStream>>
}


impl Request {

    /// Create a new Request, reading the StartLine and Headers so they are
    /// immediately useful.
    pub fn new(mut tcp: TcpStream) -> HttpResult<Request> {
        let remote_addr = try_io!(tcp.peer_name());
        let mut tcp = BufferedReader::new(tcp);
        let (method, uri, version) = try!(read_request_line(&mut tcp));
        let mut headers = try!(Headers::from_raw(&mut tcp));

        debug!("{} {} {}", method, uri, version);
        debug!("{}", headers);


        // TODO: handle Transfer-Encoding
        let body = if headers.has::<ContentLength>() {
            match headers.get_ref::<ContentLength>() {
                Some(&ContentLength(len)) => SizedReader(tcp, len),
                None => unreachable!()
            }
        } else {
            ChunkedReader(tcp, None)
        };

        Ok(Request {
            remote_addr: remote_addr,
            method: method,
            uri: uri,
            headers: headers,
            version: version,
            body: body,
        })
    }
}

impl Reader for Request {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<uint> {
        self.body.read(buf)
    }
}

