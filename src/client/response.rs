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
        debug!("{} {}", version, status);

        let headers = try!(header::Headers::from_raw(&mut stream));
        debug!("Headers: [\n{}]", headers);

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
    fn read(&mut self, buf: &mut [u8]) -> IoResult<uint> {
        self.body.read(buf)
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow::Borrowed;
    use std::boxed::BoxAny;
    use std::io::BufferedReader;

    use header::Headers;
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

        let b = res.into_inner().downcast::<MockStream>().unwrap();
        assert_eq!(b, box MockStream::new());

    }
}
