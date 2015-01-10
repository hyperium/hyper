//! Client Responses
use std::num::FromPrimitive;
use std::io::{BufferedReader, IoResult};

use header;
use header::common::{ContentLength, TransferEncoding};
use header::common::transfer_encoding::Encoding::Chunked;
use net::{NetworkStream, HttpStream};
use http::{read_status_line, HttpReader, RawStatus};
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
    body: HttpReader<BufferedReader<Box<NetworkStream + Send>>>,
}

impl Response {

    /// Creates a new response from a server.
    pub fn new(stream: Box<NetworkStream + Send>) -> HttpResult<Response> {
        let mut stream = BufferedReader::new(stream);
        let (version, raw_status) = try!(read_status_line(&mut stream));
        let status = match FromPrimitive::from_u16(raw_status.0) {
            Some(status) => status,
            None => return Err(HttpStatusError)
        };
        debug!("{:?} {:?}", version, status);

        let headers = try!(header::Headers::from_raw(&mut stream));
        debug!("Headers: [\n{:?}]", headers);

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
            version: version,
            headers: headers,
            body: body,
            status_raw: raw_status,
        })
    }

    /// Get the raw status code and reason.
    pub fn status_raw(&self) -> &RawStatus {
        &self.status_raw
    }

    /// Consumes the Request to return the NetworkStream underneath.
    pub fn into_inner(self) -> Box<NetworkStream + Send> {
        self.body.unwrap().into_inner()
    }
}

impl Reader for Response {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        self.body.read(buf)
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow::Borrowed;
    use std::boxed::BoxAny;
    use std::io::BufferedReader;

    use header::Headers;
    use header::common::TransferEncoding;
    use header::common::transfer_encoding::Encoding;
    use http::HttpReader::EofReader;
    use http::RawStatus;
    use mock::MockStream;
    use net::NetworkStream;
    use status;
    use version;

    use super::Response;


    #[test]
    fn test_unwrap() {
        let res = Response {
            status: status::StatusCode::Ok,
            headers: Headers::new(),
            version: version::HttpVersion::Http11,
            body: EofReader(BufferedReader::new(box MockStream::new() as Box<NetworkStream + Send>)),
            status_raw: RawStatus(200, Borrowed("OK"))
        };

        let b = res.into_inner().downcast::<MockStream>().ok().unwrap();
        assert_eq!(b, box MockStream::new());

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

        let mut res = Response::new(box stream).unwrap();

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
        let body = res.read_to_string().unwrap();
        assert_eq!("qwert", body);
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

        let mut res = Response::new(box stream).unwrap();

        assert!(res.read_to_string().is_err());
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

        let mut res = Response::new(box stream).unwrap();

        assert!(res.read_to_string().is_err());
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

        let mut res = Response::new(box stream).unwrap();

        assert_eq!("1", res.read_to_string().unwrap())
    }
}
