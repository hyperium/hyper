//! Client Responses
use std::io::{BufferedReader, IoResult};

use header;
use header::common::{ContentLength, TransferEncoding};
use header::common::transfer_encoding::Chunked;
use net::{NetworkStream, HttpStream};
use rfc7230::{read_status_line, HttpReader, SizedReader, ChunkedReader, EofReader};
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
    body: HttpReader<BufferedReader<S>>,
}

impl<S: NetworkStream> Response<S> {

    /// Creates a new response from a server.
    pub fn new(stream: S) -> HttpResult<Response<S>> {
        let mut stream = BufferedReader::new(stream);
        let (version, status) = try!(read_status_line(&mut stream));
        let mut headers = try!(header::Headers::from_raw(&mut stream));

        debug!("{} {}", version, status);
        debug!("{}", headers);

        let body = if headers.has::<TransferEncoding>() {
            match headers.get_ref::<TransferEncoding>() {
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
            match headers.get_ref::<ContentLength>() {
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
}

impl<S: NetworkStream> Reader for Response<S> {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> IoResult<uint> {
        self.body.read(buf)
    }
}
