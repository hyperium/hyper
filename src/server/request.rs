//! Server Requests
//!
//! These are requests that a `hyper::Server` receives, and include its method,
//! target URI, headers, and message body.
//use std::net::SocketAddr;

use version::HttpVersion;
use method::Method;
use header::Headers;
use http::{RequestHead, MessageHead, RequestLine};
use uri::RequestUri;

pub fn new(incoming: RequestHead) -> Request {
    let MessageHead { version, subject: RequestLine(method, uri), headers } = incoming;
    debug!("Request Line: {:?} {:?} {:?}", method, uri, version);
    debug!("{:#?}", headers);

    Request {
        //remote_addr: addr,
        method: method,
        uri: uri,
        headers: headers,
        version: version,
    }
}

/// A request bundles several parts of an incoming `NetworkStream`, given to a `Handler`.
#[derive(Debug)]
pub struct Request {
    // The IP address of the remote connection.
    //remote_addr: SocketAddr,
    method: Method,
    headers: Headers,
    uri: RequestUri,
    version: HttpVersion,
}


impl Request {
    /// The `Method`, such as `Get`, `Post`, etc.
    #[inline]
    pub fn method(&self) -> &Method { &self.method }

    /// The headers of the incoming request.
    #[inline]
    pub fn headers(&self) -> &Headers { &self.headers }

    /// The target request-uri for this request.
    #[inline]
    pub fn uri(&self) -> &RequestUri { &self.uri }

    /// The version of HTTP for this request.
    #[inline]
    pub fn version(&self) -> &HttpVersion { &self.version }

    /*
    pub fn path(&self) -> Option<&str> {
        match *self.uri {
            RequestUri::AbsolutePath(ref s) => Some(s),
            RequestUri::AbsoluteUri(ref url) => (),
            _ => None
        }
    }

    pub fn on_read<T: ::http::Read + Send + 'static>(self, callback: T) {
        self.body.read(callback);
    }

    pub fn read<F>(self, callback: F) where F: FnOnce(::Result<(&[u8], Self)>) + Send + 'static {
        let stream = self.body.clone();
        stream.read(::http::events::ReadOnce::new(move |result| {
            callback(result.map(move |data| (data, self)))
        }));
    }

    */
}

/*
impl fmt::Debug for Request {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Request")
            .field("method", &self.method)
            .field("uri", &self.uri)
            .field("version", &self.version)
            .field("headers", &self.headers)
            .finish()
    }
}
*/

#[cfg(test)]
mod tests {

    /*
    /// Tests that when a chunk size is not a valid radix-16 number, an error
    /// is returned.
    #[test]
    fn test_invalid_chunk_size_not_hex_digit() {
        let mut mock = MockStream::with_input(b"\
            POST / HTTP/1.1\r\n\
            Host: example.domain\r\n\
            Transfer-Encoding: chunked\r\n\
            \r\n\
            X\r\n\
            1\r\n\
            0\r\n\
            \r\n"
        );

        // FIXME: Use Type ascription
        let mock: &mut NetworkStream = &mut mock;
        let mut stream = BufReader::new(mock);

        let req = Request::new(&mut stream, sock("127.0.0.1:80")).unwrap();

        assert!(read_to_string(req).is_err());
    }

    /// Tests that when a chunk size contains an invalid extension, an error is
    /// returned.
    #[test]
    fn test_invalid_chunk_size_extension() {
        let mut mock = MockStream::with_input(b"\
            POST / HTTP/1.1\r\n\
            Host: example.domain\r\n\
            Transfer-Encoding: chunked\r\n\
            \r\n\
            1 this is an invalid extension\r\n\
            1\r\n\
            0\r\n\
            \r\n"
        );

        // FIXME: Use Type ascription
        let mock: &mut NetworkStream = &mut mock;
        let mut stream = BufReader::new(mock);

        let req = Request::new(&mut stream, sock("127.0.0.1:80")).unwrap();

        assert!(read_to_string(req).is_err());
    }

    /// Tests that when a valid extension that contains a digit is appended to
    /// the chunk size, the chunk is correctly read.
    #[test]
    fn test_chunk_size_with_extension() {
        let mut mock = MockStream::with_input(b"\
            POST / HTTP/1.1\r\n\
            Host: example.domain\r\n\
            Transfer-Encoding: chunked\r\n\
            \r\n\
            1;this is an extension with a digit 1\r\n\
            1\r\n\
            0\r\n\
            \r\n"
        );

        // FIXME: Use Type ascription
        let mock: &mut NetworkStream = &mut mock;
        let mut stream = BufReader::new(mock);

        let req = Request::new(&mut stream, sock("127.0.0.1:80")).unwrap();

        assert_eq!(read_to_string(req).unwrap(), "1".to_owned());
    }*/

}
