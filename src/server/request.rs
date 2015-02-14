//! Server Requests
//!
//! These are requests that a `hyper::Server` receives, and include its method,
//! target URI, headers, and message body.
use std::old_io::{self, IoResult};
use std::old_io::net::ip::SocketAddr;

use {HttpResult};
use version::{HttpVersion};
use method::Method::{self, Get, Head};
use header::{Headers, ContentLength, TransferEncoding};
use http::{read_request_line};
use http::HttpReader;
use http::HttpReader::{SizedReader, ChunkedReader};
use uri::RequestUri;

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
    body: Body<HttpReader<&'a mut (Reader + 'a)>>
}


impl<'a> Request<'a> {
    /// Create a new Request, reading the StartLine and Headers so they are
    /// immediately useful.
    pub fn new(mut stream: &'a mut (Reader + 'a), addr: SocketAddr) -> HttpResult<Request<'a>> {
        let (method, uri, version) = try!(read_request_line(&mut stream));
        debug!("Request Line: {:?} {:?} {:?}", method, uri, version);
        let headers = try!(Headers::from_raw(&mut stream));
        debug!("{:?}", headers);

        let body = if let Some(len) = headers.get::<ContentLength>() {
            SizedReader(stream, **len)
        } else if headers.has::<TransferEncoding>() {
            todo!("check for Transfer-Encoding: chunked");
            ChunkedReader(stream, None)
        } else {
            SizedReader(stream, 0)
        };

        let body = if method == Get || method == Head {
            Body::Empty(body)
        } else {
            Body::NonEmpty(body)
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

    /// Deconstruct a Request into its constituent parts.
    pub fn deconstruct(self) -> (SocketAddr, Method, Headers,
                                 RequestUri, HttpVersion,
                                 HttpReader<&'a mut (Reader + 'a)>,) {
        (self.remote_addr, self.method, self.headers,
         self.uri, self.version, self.body.into_inner())
    }
}

impl<'a> Reader for Request<'a> {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        match self.body {
            Body::Empty(..) => Err(old_io::standard_error(old_io::EndOfFile)),
            Body::NonEmpty(ref mut r) => r.read(buf)
        }
    }
}

enum Body<R> {
    Empty(R),
    NonEmpty(R),
}

impl<R> Body<R> {
    fn into_inner(self) -> R {
        match self {
            Body::Empty(r) => r,
            Body::NonEmpty(r) => r
        }
    }
}

#[cfg(test)]
mod tests {
    use header::{Host, TransferEncoding, Encoding};
    use mock::MockStream;
    use super::Request;

    use std::old_io::net::ip::SocketAddr;

    fn sock(s: &str) -> SocketAddr {
        s.parse().unwrap()
    }

    #[test]
    fn test_get_empty_body() {
        let mut stream = MockStream::with_input(b"\
            GET / HTTP/1.1\r\n\
            Host: example.domain\r\n\
            Content-Length: 18\r\n\
            \r\n\
            I'm a bad request.\
        ");

        let mut req = Request::new(&mut stream, sock("127.0.0.1:80")).unwrap();
        assert_eq!(req.read_to_string(), Ok("".to_string()));
    }

    #[test]
    fn test_head_empty_body() {
        let mut stream = MockStream::with_input(b"\
            HEAD / HTTP/1.1\r\n\
            Host: example.domain\r\n\
            Content-Length: 18\r\n\
            \r\n\
            I'm a bad request.\
        ");

        let mut req = Request::new(&mut stream, sock("127.0.0.1:80")).unwrap();
        assert_eq!(req.read_to_string(), Ok("".to_string()));
    }

    #[test]
    fn test_post_body_with_no_content_length() {
        let mut stream = MockStream::with_input(b"\
            POST / HTTP/1.1\r\n\
            Host: example.domain\r\n\
            \r\n\
            I'm a bad request.\r\n\
        ");

        let mut req = Request::new(&mut stream, sock("127.0.0.1:80")).unwrap();
        assert_eq!(req.read_to_string(), Ok("".to_string()));
    }

    #[test]
    fn test_unexpected_body_drains_upon_drop() {
        let mut stream = MockStream::with_input(b"\
            GET / HTTP/1.1\r\n\
            Host: example.domain\r\n\
            Content-Length: 18\r\n\
            \r\n\
            I'm a bad request.\
        ");

        Request::new(&mut stream, sock("127.0.0.1:80")).unwrap().read_to_string().unwrap();
        assert!(stream.read.eof());
    }

    #[test]
    fn test_parse_chunked_request() {
        let mut stream = MockStream::with_input(b"\
            POST / HTTP/1.1\r\n\
            Host: example.domain\r\n\
            Transfer-Encoding: chunked\r\n\
            \r\n\
            1\r\n\
            q\r\n\
            2\r\n\
            we\r\n\
            2\r\n\
            rt\r\n\
            0\r\n\
            \r\n"
        );

        let mut req = Request::new(&mut stream, sock("127.0.0.1:80")).unwrap();

        // The headers are correct?
        match req.headers.get::<Host>() {
            Some(host) => {
                assert_eq!("example.domain", host.hostname);
            },
            None => panic!("Host header expected!"),
        };
        match req.headers.get::<TransferEncoding>() {
            Some(encodings) => {
                assert_eq!(1, encodings.len());
                assert_eq!(Encoding::Chunked, encodings[0]);
            }
            None => panic!("Transfer-Encoding: chunked expected!"),
        };
        // The content is correctly read?
        let body = req.read_to_string().unwrap();
        assert_eq!("qwert", body);
    }

    /// Tests that when a chunk size is not a valid radix-16 number, an error
    /// is returned.
    #[test]
    fn test_invalid_chunk_size_not_hex_digit() {
        let mut stream = MockStream::with_input(b"\
            POST / HTTP/1.1\r\n\
            Host: example.domain\r\n\
            Transfer-Encoding: chunked\r\n\
            \r\n\
            X\r\n\
            1\r\n\
            0\r\n\
            \r\n"
        );

        let mut req = Request::new(&mut stream, sock("127.0.0.1:80")).unwrap();

        assert!(req.read_to_string().is_err());
    }

    /// Tests that when a chunk size contains an invalid extension, an error is
    /// returned.
    #[test]
    fn test_invalid_chunk_size_extension() {
        let mut stream = MockStream::with_input(b"\
            POST / HTTP/1.1\r\n\
            Host: example.domain\r\n\
            Transfer-Encoding: chunked\r\n\
            \r\n\
            1 this is an invalid extension\r\n\
            1\r\n\
            0\r\n\
            \r\n"
        );

        let mut req = Request::new(&mut stream, sock("127.0.0.1:80")).unwrap();

        assert!(req.read_to_string().is_err());
    }

    /// Tests that when a valid extension that contains a digit is appended to
    /// the chunk size, the chunk is correctly read.
    #[test]
    fn test_chunk_size_with_extension() {
        let mut stream = MockStream::with_input(b"\
            POST / HTTP/1.1\r\n\
            Host: example.domain\r\n\
            Transfer-Encoding: chunked\r\n\
            \r\n\
            1;this is an extension with a digit 1\r\n\
            1\r\n\
            0\r\n\
            \r\n"
        );

        let mut req = Request::new(&mut stream, sock("127.0.0.1:80")).unwrap();

        assert_eq!("1", req.read_to_string().unwrap())
    }

}
