//! Server Responses
//!
//! These are responses sent by a `hyper::Server` to clients, after
//! receiving a request.
use std::any::{Any, TypeId};
use std::marker::PhantomData;
use std::mem;
use std::io::{self, Write};
use std::ptr;

use time::now_utc;

use header;
use http::h1::{CR, LF, LINE_ENDING, HttpWriter};
use http::h1::HttpWriter::{ThroughWriter, ChunkedWriter, SizedWriter};
use status;
use net::{Fresh, Streaming};
use version;


/// The outgoing half for a Tcp connection, created by a `Server` and given to a `Handler`.
///
/// The default `StatusCode` for a `Response` is `200 OK`.
///
/// There is a `Drop` implementation for `Response` that will automatically
/// write the head and flush the body, if the handler has not already done so,
/// so that the server doesn't accidentally leave dangling requests.
#[derive(Debug)]
pub struct Response<'a, W: Any = Fresh> {
    /// The HTTP version of this response.
    pub version: version::HttpVersion,
    // Stream the Response is writing to, not accessible through UnwrittenResponse
    body: HttpWriter<&'a mut (Write + 'a)>,
    // The status code for the request.
    status: status::StatusCode,
    // The outgoing headers on this response.
    headers: &'a mut header::Headers,

    _writing: PhantomData<W>
}

impl<'a, W: Any> Response<'a, W> {
    /// The status of this response.
    #[inline]
    pub fn status(&self) -> status::StatusCode { self.status }

    /// The headers of this response.
    #[inline]
    pub fn headers(&self) -> &header::Headers { &*self.headers }

    /// Construct a Response from its constituent parts.
    #[inline]
    pub fn construct(version: version::HttpVersion,
                     body: HttpWriter<&'a mut (Write + 'a)>,
                     status: status::StatusCode,
                     headers: &'a mut header::Headers) -> Response<'a, Fresh> {
        Response {
            status: status,
            version: version,
            body: body,
            headers: headers,
            _writing: PhantomData,
        }
    }

    /// Deconstruct this Response into its constituent parts.
    #[inline]
    pub fn deconstruct(self) -> (version::HttpVersion, HttpWriter<&'a mut (Write + 'a)>,
                                 status::StatusCode, &'a mut header::Headers) {
        unsafe {
            let parts = (
                self.version,
                ptr::read(&self.body),
                self.status,
                ptr::read(&self.headers)
            );
            mem::forget(self);
            parts
        }
    }

    fn write_head(&mut self) -> io::Result<Body> {
        debug!("writing head: {:?} {:?}", self.version, self.status);
        try!(write!(&mut self.body, "{} {}{}{}", self.version, self.status,
            CR as char, LF as char));

        if !self.headers.has::<header::Date>() {
            self.headers.set(header::Date(header::HttpDate(now_utc())));
        }


        let mut body_type = Body::Chunked;

        if let Some(cl) = self.headers.get::<header::ContentLength>() {
            body_type = Body::Sized(**cl);
        };

        // can't do in match above, thanks borrowck
        if body_type == Body::Chunked {
            let encodings = match self.headers.get_mut::<header::TransferEncoding>() {
                Some(&mut header::TransferEncoding(ref mut encodings)) => {
                    //TODO: check if chunked is already in encodings. use HashSet?
                    encodings.push(header::Encoding::Chunked);
                    false
                },
                None => true
            };

            if encodings {
                self.headers.set::<header::TransferEncoding>(
                    header::TransferEncoding(vec![header::Encoding::Chunked]))
            }
        }


        debug!("headers [\n{:?}]", self.headers);
        try!(write!(&mut self.body, "{}", self.headers));
        try!(write!(&mut self.body, "{}", LINE_ENDING));

        Ok(body_type)
    }
}

impl<'a> Response<'a, Fresh> {
    /// Creates a new Response that can be used to write to a network stream.
    #[inline]
    pub fn new(stream: &'a mut (Write + 'a), headers: &'a mut header::Headers) ->
            Response<'a, Fresh> {
        Response {
            status: status::StatusCode::Ok,
            version: version::HttpVersion::Http11,
            headers: headers,
            body: ThroughWriter(stream),
            _writing: PhantomData,
        }
    }

    /// Writes the body and ends the response.
    ///
    /// This is a shortcut method for when you have a response with a fixed
    /// size, and would only need a single `write` call normally.
    ///
    /// # Example
    ///
    /// ```
    /// # use hyper::server::Response;
    /// fn handler(res: Response) {
    ///     res.send(b"Hello World!").unwrap();
    /// }
    /// ```
    ///
    /// The above is the same, but shorter, than the longer:
    ///
    /// ```
    /// # use hyper::server::Response;
    /// use std::io::Write;
    /// use hyper::header::ContentLength;
    /// fn handler(mut res: Response) {
    ///     let body = b"Hello World!";
    ///     res.headers_mut().set(ContentLength(body.len() as u64));
    ///     let mut res = res.start().unwrap();
    ///     res.write_all(body).unwrap();
    /// }
    /// ```
    #[inline]
    pub fn send(mut self, body: &[u8]) -> io::Result<()> {
        self.headers.set(header::ContentLength(body.len() as u64));
        let mut stream = try!(self.start());
        try!(stream.write_all(body));
        stream.end()
    }

    /// Consume this Response<Fresh>, writing the Headers and Status and
    /// creating a Response<Streaming>
    pub fn start(mut self) -> io::Result<Response<'a, Streaming>> {
        let body_type = try!(self.write_head());
        let (version, body, status, headers) = self.deconstruct();
        let stream = match body_type {
            Body::Chunked => ChunkedWriter(body.into_inner()),
            Body::Sized(len) => SizedWriter(body.into_inner(), len)
        };

        // "copy" to change the phantom type
        Ok(Response {
            version: version,
            body: stream,
            status: status,
            headers: headers,
            _writing: PhantomData,
        })
    }
    /// Get a mutable reference to the status.
    #[inline]
    pub fn status_mut(&mut self) -> &mut status::StatusCode { &mut self.status }

    /// Get a mutable reference to the Headers.
    #[inline]
    pub fn headers_mut(&mut self) -> &mut header::Headers { self.headers }
}


impl<'a> Response<'a, Streaming> {
    /// Flushes all writing of a response to the client.
    #[inline]
    pub fn end(self) -> io::Result<()> {
        trace!("ending");
        let (_, body, _, _) = self.deconstruct();
        try!(body.end());
        Ok(())
    }
}

impl<'a> Write for Response<'a, Streaming> {
    #[inline]
    fn write(&mut self, msg: &[u8]) -> io::Result<usize> {
        debug!("write {:?} bytes", msg.len());
        self.body.write(msg)
    }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        self.body.flush()
    }
}

#[derive(PartialEq)]
enum Body {
    Chunked,
    Sized(u64),
}

impl<'a, T: Any> Drop for Response<'a, T> {
    fn drop(&mut self) {
        if TypeId::of::<T>() == TypeId::of::<Fresh>() {
            let mut body = match self.write_head() {
                Ok(Body::Chunked) => ChunkedWriter(self.body.get_mut()),
                Ok(Body::Sized(len)) => SizedWriter(self.body.get_mut(), len),
                Err(e) => {
                    debug!("error dropping request: {:?}", e);
                    return;
                }
            };
            end(&mut body);
        } else {
            end(&mut self.body);
        };


        #[inline]
        fn end<W: Write>(w: &mut W) {
            match w.write(&[]) {
                Ok(_) => match w.flush() {
                    Ok(_) => debug!("drop successful"),
                    Err(e) => debug!("error dropping request: {:?}", e)
                },
                Err(e) => debug!("error dropping request: {:?}", e)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use header::Headers;
    use mock::MockStream;
    use super::Response;

    macro_rules! lines {
        ($s:ident = $($line:pat),+) => ({
            let s = String::from_utf8($s.write).unwrap();
            let mut lines = s.split_terminator("\r\n");

            $(
                match lines.next() {
                    Some($line) => (),
                    other => panic!("line mismatch: {:?} != {:?}", other, stringify!($line))
                }
            )+

            assert_eq!(lines.next(), None);
        })
    }

    #[test]
    fn test_fresh_start() {
        let mut headers = Headers::new();
        let mut stream = MockStream::new();
        {
            let res = Response::new(&mut stream, &mut headers);
            res.start().unwrap().deconstruct();
        }

        lines! { stream =
            "HTTP/1.1 200 OK",
            _date,
            _transfer_encoding,
            ""
        }
    }

    #[test]
    fn test_streaming_end() {
        let mut headers = Headers::new();
        let mut stream = MockStream::new();
        {
            let res = Response::new(&mut stream, &mut headers);
            res.start().unwrap().end().unwrap();
        }

        lines! { stream =
            "HTTP/1.1 200 OK",
            _date,
            _transfer_encoding,
            "",
            "0",
            "" // empty zero body
        }
    }

    #[test]
    fn test_fresh_drop() {
        use status::StatusCode;
        let mut headers = Headers::new();
        let mut stream = MockStream::new();
        {
            let mut res = Response::new(&mut stream, &mut headers);
            *res.status_mut() = StatusCode::NotFound;
        }

        lines! { stream =
            "HTTP/1.1 404 Not Found",
            _date,
            _transfer_encoding,
            "",
            "0",
            "" // empty zero body
        }
    }

    #[test]
    fn test_streaming_drop() {
        use std::io::Write;
        use status::StatusCode;
        let mut headers = Headers::new();
        let mut stream = MockStream::new();
        {
            let mut res = Response::new(&mut stream, &mut headers);
            *res.status_mut() = StatusCode::NotFound;
            let mut stream = res.start().unwrap();
            stream.write_all(b"foo").unwrap();
        }

        lines! { stream =
            "HTTP/1.1 404 Not Found",
            _date,
            _transfer_encoding,
            "",
            "3",
            "foo",
            "0",
            "" // empty zero body
        }
    }
}
