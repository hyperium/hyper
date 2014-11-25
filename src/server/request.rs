//! Server Requests
//!
//! These are requests that a `hyper::Server` receives, and include its method,
//! target URI, headers, and message body.
use std::io::IoResult;
use std::io::net::ip::SocketAddr;

use {HttpResult};
use version::{HttpVersion};
use method::Method::{mod, Get, Head};
use header::Headers;
use header::common::{ContentLength, TransferEncoding};
use http::{read_request_line};
use http::HttpReader;
use http::HttpReader::{SizedReader, ChunkedReader, EmptyReader};
use uri::RequestUri;

pub type InternalReader<'a> = &'a mut Reader + 'a;

/// A request bundles several parts of an incoming `NetworkStream`, given to a `Handler`.
pub struct Request<'a> {
    /// The IP address of the remote connection.
    pub remote_addr: SocketAddr,
    /// The `Method`, such as `Get`, `Post`, etc.
    pub method: Method,
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


        let body = if method == Get || method == Head {
            EmptyReader(stream)
        } else if headers.has::<ContentLength>() {
            match headers.get::<ContentLength>() {
                Some(&ContentLength(len)) => SizedReader(stream, len),
                None => unreachable!()
            }
        } else if headers.has::<TransferEncoding>() {
            todo!("check for Transfer-Encoding: chunked");
            ChunkedReader(stream, None)
        } else {
            EmptyReader(stream)
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

#[cfg(test)]
mod tests {
    use mock::MockStream;
    use super::Request;

    macro_rules! sock(
        ($s:expr) => (::std::str::from_str::<::std::io::net::ip::SocketAddr>($s).unwrap())
    )

    #[test]
    fn test_get_empty_body() {
        let mut stream = MockStream::with_input(b"\
            GET / HTTP/1.1\r\n\
            Host: example.domain\r\n\
            \r\n\
            I'm a bad request.\r\n\
        ");

        let mut req = Request::new(&mut stream, sock!("127.0.0.1:80")).unwrap();
        assert_eq!(req.read_to_string(), Ok("".into_string()));
    }

    #[test]
    fn test_head_empty_body() {
        let mut stream = MockStream::with_input(b"\
            HEAD / HTTP/1.1\r\n\
            Host: example.domain\r\n\
            \r\n\
            I'm a bad request.\r\n\
        ");

        let mut req = Request::new(&mut stream, sock!("127.0.0.1:80")).unwrap();
        assert_eq!(req.read_to_string(), Ok("".into_string()));
    }

    #[test]
    fn test_post_empty_body() {
        let mut stream = MockStream::with_input(b"\
            POST / HTTP/1.1\r\n\
            Host: example.domain\r\n\
            \r\n\
            I'm a bad request.\r\n\
        ");

        let mut req = Request::new(&mut stream, sock!("127.0.0.1:80")).unwrap();
        assert_eq!(req.read_to_string(), Ok("".into_string()));
    }
}
