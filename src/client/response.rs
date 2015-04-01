//! Client Responses
use std::io::{self, Read};
use std::num::FromPrimitive;
use std::marker::PhantomData;

use buffer::BufReader;
use header;
use header::{ContentLength, TransferEncoding};
use header::Encoding::Chunked;
use net::{NetworkStream, HttpStream};
use http::{self, HttpReader, RawStatus};
use http::HttpReader::{SizedReader, ChunkedReader, EofReader};
use status;
use version;
use HttpResult;
use HttpError::HttpStatusError;

/// A response for a client request to a remote server.
pub struct Response<S = HttpStream> {
    /// The status from the server.
    pub status: status::StatusCode,
    /// The headers from the server.
    pub headers: header::Headers,
    /// The HTTP version of this response from the server.
    pub version: version::HttpVersion,
    status_raw: RawStatus,
    body: HttpReader<BufReader<Box<NetworkStream + Send>>>,

    _marker: PhantomData<S>,
}

impl Response {

    /// Creates a new response from a server.
    pub fn new(stream: Box<NetworkStream + Send>) -> HttpResult<Response> {
        let mut stream = BufReader::new(stream);

        let head = try!(http::parse_response(&mut stream));
        let raw_status = head.subject;
        let headers = head.headers;

        let status = match FromPrimitive::from_u16(raw_status.0) {
            Some(status) => status,
            None => return Err(HttpStatusError)
        };
        debug!("version={:?}, status={:?}", head.version, status);
        debug!("headers={:?}", headers);

        let body = if headers.has::<TransferEncoding>() {
            match headers.get::<TransferEncoding>() {
                Some(&TransferEncoding(ref codings)) => {
                    if codings.len() > 1 {
                        debug!("TODO: #2 handle other codings: {:?}", codings);
                    };

                    if codings.contains(&Chunked) {
                        ChunkedReader(stream, None)
                    } else {
                        debug!("not chuncked. read till eof");
                        EofReader(stream)
                    }
                }
                None => unreachable!()
            }
        } else if headers.has::<ContentLength>() {
            match headers.get::<ContentLength>() {
                Some(&ContentLength(len)) => SizedReader(stream, len),
                None => unreachable!()
            }
        } else {
            debug!("neither Transfer-Encoding nor Content-Length");
            EofReader(stream)
        };

        Ok(Response {
            status: status,
            version: head.version,
            headers: headers,
            body: body,
            status_raw: raw_status,
            _marker: PhantomData,
        })
    }

    /// Get the raw status code and reason.
    pub fn status_raw(&self) -> &RawStatus {
        &self.status_raw
    }

    /// Consumes the Request to return the NetworkStream underneath.
    pub fn into_inner(self) -> Box<NetworkStream + Send> {
        self.body.into_inner().into_inner()
    }
}

impl Read for Response {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.body.read(buf)
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow::Borrowed;
    use std::io::{self, Read};
    use std::marker::PhantomData;

    use buffer::BufReader;
    use header::Headers;
    use header::TransferEncoding;
    use header::Encoding;
    use http::HttpReader::EofReader;
    use http::RawStatus;
    use mock::MockStream;
    use status;
    use version;

    use super::Response;

    fn read_to_string(mut r: Response) -> io::Result<String> {
        let mut s = String::new();
        try!(r.read_to_string(&mut s));
        Ok(s)
    }


    #[test]
    fn test_into_inner() {
        let res = Response {
            status: status::StatusCode::Ok,
            headers: Headers::new(),
            version: version::HttpVersion::Http11,
            body: EofReader(BufReader::new(Box::new(MockStream::new()))),
            status_raw: RawStatus(200, Borrowed("OK")),
            _marker: PhantomData,
        };

        let b = res.into_inner().downcast::<MockStream>().ok().unwrap();
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

        let res = Response::new(Box::new(stream)).unwrap();

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
        assert_eq!(read_to_string(res), Ok("qwert".to_string()));
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

        let res = Response::new(Box::new(stream)).unwrap();

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

        let res = Response::new(Box::new(stream)).unwrap();

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

        let res = Response::new(Box::new(stream)).unwrap();

        assert_eq!(read_to_string(res), Ok("1".to_string()));
    }
}
