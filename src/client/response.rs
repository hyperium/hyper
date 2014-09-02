//! # Client Responses
use std::io::{Reader, IoResult};
use std::io::net::tcp::TcpStream;

use header::{mod, ContentLength, TransferEncoding, Chunked};
use rfc7230::{read_status_line, HttpReader, SizedReader, ChunkedReader, EofReader};
use status;
use version;
use {HttpResult};

/// A response for a client request to a remote server.
pub struct Response {
    /// The status from the server.
    pub status: status::StatusCode,
    /// The headers from the server.
    pub headers: header::Headers,
    /// The HTTP version of this response from the server.
    pub version: version::HttpVersion,
    body: HttpReader<TcpStream>,
}

impl Response {

    /// Creates a new response from a server.
    pub fn new(mut tcp: TcpStream) -> HttpResult<Response> {
        let (version, status) = try!(read_status_line(&mut tcp));
        let mut headers = try!(header::Headers::from_raw(&mut tcp));

        debug!("{} {}", version, status);
        debug!("{}", headers);

        let body = if headers.has::<TransferEncoding>() {
            match headers.get_ref::<TransferEncoding>() {
                Some(&TransferEncoding(ref codings)) => {
                    if codings.len() > 1 {
                        debug!("TODO: handle other codings: {}", codings);
                    };

                    if codings.contains(&Chunked) {
                        ChunkedReader(tcp, None)
                    } else {
                        debug!("not chucked. read till eof");
                        EofReader(tcp)
                    }
                }
                None => unreachable!()
            }
        } else if headers.has::<ContentLength>() {
            match headers.get_ref::<ContentLength>() {
                Some(&ContentLength(len)) => SizedReader(tcp, len),
                None => unreachable!()
            }
        } else {
            debug!("neither Transfer-Encoding nor Content-Length");
            EofReader(tcp)
        };

        Ok(Response {
            status: status,
            version: version,
            headers: headers,
            body: body,
        })
    }
}

impl Reader for Response {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<uint> {
        self.body.read(buf)
    }
}
