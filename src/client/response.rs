//! Client Responses
use std::io::{BufferedReader, IoResult};

use header;
use header::common::{ContentLength, TransferEncoding};
use header::common::transfer_encoding::Chunked;
use net::{NetworkStream, HttpStream};
use http::{read_status_line, HttpReader, SizedReader, ChunkedReader, EofReader};
use status;
use version;
use {HttpResult};

/// A response for a client request to a remote server.
pub struct Response<S = HttpStream> {
    /// The status from the server.
    pub status: status::StatusCode,
    /// The headers from the server.
    pub headers: header::Headers,
    /// The HTTP version of this response from the server.
    pub version: version::HttpVersion,
    body: HttpReader<BufferedReader<Box<NetworkStream + Send>>>,
}

impl Response {

    /// Creates a new response from a server.
    pub fn new(stream: Box<NetworkStream + Send>) -> HttpResult<Response> {
        let mut stream = BufferedReader::new(stream.dynamic());
        let (version, status) = try!(read_status_line(&mut stream));
        let headers = try!(header::Headers::from_raw(&mut stream));

        debug!("{} {}", version, status);
        debug!("{}", headers);

        let body = if headers.has::<TransferEncoding>() {
            match headers.get::<TransferEncoding>() {
                Some(&TransferEncoding(ref codings)) => {
                    if codings.len() > 1 {
                        debug!("TODO: #2 handle other codings: {}", codings);
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
        })
    }

    /// Unwraps the Request to return the NetworkStream underneath.
    pub fn unwrap(self) -> Box<NetworkStream + Send> {
        self.body.unwrap().unwrap()
    }
}

impl Reader for Response {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> IoResult<uint> {
        self.body.read(buf)
    }
}

#[cfg(test)]
mod tests {
    use std::boxed::BoxAny;
    use std::io::BufferedReader;

    use header::Headers;
    use http::EofReader;
    use mock::MockStream;
    use net::NetworkStream;
    use status;
    use version;

    use super::Response;


    #[test]
    fn test_unwrap() {
        let res = Response {
            status: status::Ok,
            headers: Headers::new(),
            version: version::Http11,
            body: EofReader(BufferedReader::new(box MockStream as Box<NetworkStream + Send>))
        };

        let b = res.unwrap().downcast::<MockStream>().unwrap();
        assert_eq!(b, box MockStream);

    }
}
