//! Client Requests
use std::old_io::{BufferedWriter, IoResult};

use url::Url;

use method::{self, Method};
use header::Headers;
use header::{self, Host};
use net::{NetworkStream, NetworkConnector, HttpConnector, Fresh, Streaming};
use http::{HttpWriter, LINE_ENDING};
use http::HttpWriter::{ThroughWriter, ChunkedWriter, SizedWriter, EmptyWriter};
use version;
use HttpResult;
use client::{Response, get_host_and_port};


/// A client request to a remote server.
pub struct Request<W> {
    /// The target URI for this request.
    pub url: Url,

    /// The HTTP version of this request.
    pub version: version::HttpVersion,

    body: HttpWriter<BufferedWriter<Box<NetworkStream + Send>>>,
    headers: Headers,
    method: method::Method,
}

impl<W> Request<W> {
    /// Read the Request headers.
    #[inline]
    pub fn headers(&self) -> &Headers { &self.headers }

    /// Read the Request method.
    #[inline]
    pub fn method(&self) -> method::Method { self.method.clone() }
}

impl Request<Fresh> {
    /// Create a new client request.
    pub fn new(method: method::Method, url: Url) -> HttpResult<Request<Fresh>> {
        let mut conn = HttpConnector(None);
        Request::with_connector(method, url, &mut conn)
    }

    /// Create a new client request with a specific underlying NetworkStream.
    pub fn with_connector<C, S>(method: method::Method, url: Url, connector: &mut C)
        -> HttpResult<Request<Fresh>> where
        C: NetworkConnector<Stream=S>,
        S: NetworkStream + Send {
        debug!("{} {}", method, url);
        let (host, port) = try!(get_host_and_port(&url));

        let stream = try!(connector.connect(&*host, port, &*url.scheme));
        let stream = ThroughWriter(BufferedWriter::new(box stream as Box<NetworkStream + Send>));

        let mut headers = Headers::new();
        headers.set(Host {
            hostname: host,
            port: Some(port),
        });

        Ok(Request {
            method: method,
            headers: headers,
            url: url,
            version: version::HttpVersion::Http11,
            body: stream
        })
    }

    /// Consume a Fresh Request, writing the headers and method,
    /// returning a Streaming Request.
    pub fn start(mut self) -> HttpResult<Request<Streaming>> {
        let mut uri = self.url.serialize_path().unwrap();
        //TODO: this needs a test
        if let Some(ref q) = self.url.query {
            uri.push('?');
            uri.push_str(&q[..]);
        }

        debug!("writing head: {:?} {:?} {:?}", self.method, uri, self.version);
        try!(write!(&mut self.body, "{} {} {}{}",
                    self.method, uri, self.version, LINE_ENDING));


        let stream = match self.method {
            Method::Get | Method::Head => {
                debug!("headers [\n{:?}]", self.headers);
                try!(write!(&mut self.body, "{}{}", self.headers, LINE_ENDING));
                EmptyWriter(self.body.unwrap())
            },
            _ => {
                let mut chunked = true;
                let mut len = 0;

                match self.headers.get::<header::ContentLength>() {
                    Some(cl) => {
                        chunked = false;
                        len = **cl;
                    },
                    None => ()
                };

                // cant do in match above, thanks borrowck
                if chunked {
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
                try!(write!(&mut self.body, "{}{}", self.headers, LINE_ENDING));

                if chunked {
                    ChunkedWriter(self.body.unwrap())
                } else {
                    SizedWriter(self.body.unwrap(), len)
                }
            }
        };

        Ok(Request {
            method: self.method,
            headers: self.headers,
            url: self.url,
            version: self.version,
            body: stream
        })
    }

    /// Get a mutable reference to the Request headers.
    #[inline]
    pub fn headers_mut(&mut self) -> &mut Headers { &mut self.headers }
}

impl Request<Streaming> {
    /// Completes writing the request, and returns a response to read from.
    ///
    /// Consumes the Request.
    pub fn send(self) -> HttpResult<Response> {
        let raw = try!(self.body.end()).into_inner();
        Response::new(raw)
    }
}

impl Writer for Request<Streaming> {
    #[inline]
    fn write_all(&mut self, msg: &[u8]) -> IoResult<()> {
        self.body.write_all(msg)
    }

    #[inline]
    fn flush(&mut self) -> IoResult<()> {
        self.body.flush()
    }
}

#[cfg(test)]
mod tests {
    use std::boxed::BoxAny;
    use std::str::from_utf8;
    use url::Url;
    use method::Method::{Get, Head};
    use mock::{MockStream, MockConnector};
    use super::Request;

    #[test]
    fn test_get_empty_body() {
        let req = Request::with_connector(
            Get, Url::parse("http://example.dom").unwrap(), &mut MockConnector
        ).unwrap();
        let req = req.start().unwrap();
        let stream = *req.body.end().unwrap()
            .into_inner().downcast::<MockStream>().ok().unwrap();
        let bytes = stream.write.into_inner();
        let s = from_utf8(&bytes[..]).unwrap();
        assert!(!s.contains("Content-Length:"));
        assert!(!s.contains("Transfer-Encoding:"));
    }

    #[test]
    fn test_head_empty_body() {
        let req = Request::with_connector(
            Head, Url::parse("http://example.dom").unwrap(), &mut MockConnector
        ).unwrap();
        let req = req.start().unwrap();
        let stream = *req.body.end().unwrap()
            .into_inner().downcast::<MockStream>().ok().unwrap();
        let bytes = stream.write.into_inner();
        let s = from_utf8(&bytes[..]).unwrap();
        assert!(!s.contains("Content-Length:"));
        assert!(!s.contains("Transfer-Encoding:"));
    }
}
