//! Client Responses
use std::io::{self, Read};

use url::Url;

use header;
use net::NetworkStream;
use http::{self, RawStatus, ResponseHead, HttpMessage};
use http::h1::Http11Message;
use status;
use version;

/// A response for a client request to a remote server.
#[derive(Debug)]
pub struct Response {
    /// The status from the server.
    pub status: status::StatusCode,
    /// The headers from the server.
    pub headers: header::Headers,
    /// The HTTP version of this response from the server.
    pub version: version::HttpVersion,
    /// The final URL of this response.
    pub url: Url,
    status_raw: RawStatus,
    message: Box<HttpMessage>,
}

impl Response {
    /// Creates a new response from a server.
    pub fn new(url: Url, stream: Box<NetworkStream + Send>) -> ::Result<Response> {
        trace!("Response::new");
        Response::with_message(url, Box::new(Http11Message::with_stream(stream)))
    }

    /// Creates a new response received from the server on the given `HttpMessage`.
    pub fn with_message(url: Url, mut message: Box<HttpMessage>) -> ::Result<Response> {
        trace!("Response::with_message");
        let ResponseHead { headers, raw_status, version } = match message.get_incoming() {
            Ok(head) => head,
            Err(e) => {
                let _ = message.close_connection();
                return Err(From::from(e));
            }
        };
        let status = status::StatusCode::from_u16(raw_status.0);
        debug!("version={:?}, status={:?}", version, status);
        debug!("headers={:?}", headers);

        Ok(Response {
            status: status,
            version: version,
            headers: headers,
            url: url,
            status_raw: raw_status,
            message: message,
        })
    }

    /// Get the raw status code and reason.
    #[inline]
    pub fn status_raw(&self) -> &RawStatus {
        &self.status_raw
    }

    /// Gets a borrowed reference to the underlying `HttpMessage`.
    #[inline]
    pub fn get_ref(&self) -> &HttpMessage {
        &*self.message
    }
}

/// Read the response body.
impl Read for Response {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self.message.read(buf) {
            Err(e) => {
                let _ = self.message.close_connection();
                Err(e)
            }
            r => r
        }
    }
}

impl Drop for Response {
    fn drop(&mut self) {
        // if not drained, theres old bits in the Reader. we can't reuse this,
        // since those old bits would end up in new Responses
        //
        // otherwise, the response has been drained. we should check that the
        // server has agreed to keep the connection open
        let is_drained = !self.message.has_body();
        trace!("Response.drop is_drained={}", is_drained);
        if !(is_drained && http::should_keep_alive(self.version, &self.headers)) {
            trace!("Response.drop closing connection");
            if let Err(e) = self.message.close_connection() {
                info!("Response.drop error closing connection: {}", e);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::{self, Read};

    use url::Url;

    use header::TransferEncoding;
    use header::Encoding;
    use http::HttpMessage;
    use mock::MockStream;
    use status;
    use version;
    use http::h1::Http11Message;

    use super::Response;

    fn read_to_string(mut r: Response) -> io::Result<String> {
        let mut s = String::new();
        try!(r.read_to_string(&mut s));
        Ok(s)
    }


    #[test]
    fn test_into_inner() {
        let message: Box<HttpMessage> = Box::new(
            Http11Message::with_stream(Box::new(MockStream::new())));
        let message = message.downcast::<Http11Message>().ok().unwrap();
        let b = message.into_inner().downcast::<MockStream>().ok().unwrap();
        assert_eq!(b, Box::new(MockStream::new()));
    }

    #[test]
    fn test_parse_chunked_response() {
        let stream = MockStream::with_input(b"\
            HTTP/1.1 200 OK\r\n\
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

        let url = Url::parse("http://hyper.rs").unwrap();
        let res = Response::new(url, Box::new(stream)).unwrap();

        // The status line is correct?
        assert_eq!(res.status, status::StatusCode::Ok);
        assert_eq!(res.version, version::HttpVersion::Http11);
        // The header is correct?
        match res.headers.get::<TransferEncoding>() {
            Some(encodings) => {
                assert_eq!(1, encodings.len());
                assert_eq!(Encoding::Chunked, encodings[0]);
            },
            None => panic!("Transfer-Encoding: chunked expected!"),
        };
        // The body is correct?
        assert_eq!(read_to_string(res).unwrap(), "qwert".to_owned());
    }

    /// Tests that when a chunk size is not a valid radix-16 number, an error
    /// is returned.
    #[test]
    fn test_invalid_chunk_size_not_hex_digit() {
        let stream = MockStream::with_input(b"\
            HTTP/1.1 200 OK\r\n\
            Transfer-Encoding: chunked\r\n\
            \r\n\
            X\r\n\
            1\r\n\
            0\r\n\
            \r\n"
        );

        let url = Url::parse("http://hyper.rs").unwrap();
        let res = Response::new(url, Box::new(stream)).unwrap();

        assert!(read_to_string(res).is_err());
    }

    /// Tests that when a chunk size contains an invalid extension, an error is
    /// returned.
    #[test]
    fn test_invalid_chunk_size_extension() {
        let stream = MockStream::with_input(b"\
            HTTP/1.1 200 OK\r\n\
            Transfer-Encoding: chunked\r\n\
            \r\n\
            1 this is an invalid extension\r\n\
            1\r\n\
            0\r\n\
            \r\n"
        );

        let url = Url::parse("http://hyper.rs").unwrap();
        let res = Response::new(url, Box::new(stream)).unwrap();

        assert!(read_to_string(res).is_err());
    }

    /// Tests that when a valid extension that contains a digit is appended to
    /// the chunk size, the chunk is correctly read.
    #[test]
    fn test_chunk_size_with_extension() {
        let stream = MockStream::with_input(b"\
            HTTP/1.1 200 OK\r\n\
            Transfer-Encoding: chunked\r\n\
            \r\n\
            1;this is an extension with a digit 1\r\n\
            1\r\n\
            0\r\n\
            \r\n"
        );

        let url = Url::parse("http://hyper.rs").unwrap();
        let res = Response::new(url, Box::new(stream)).unwrap();

        assert_eq!(read_to_string(res).unwrap(), "1".to_owned());
    }

    #[test]
    fn test_parse_error_closes() {
        let url = Url::parse("http://hyper.rs").unwrap();
        let stream = MockStream::with_input(b"\
            definitely not http
        ");

        assert!(Response::new(url, Box::new(stream)).is_err());
    }
}
