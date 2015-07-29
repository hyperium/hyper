//! Adapts the `solicit`-provided HTTP/2 implementation into the `HttpMessage` API.

use std::io::{self, Write, Read, Cursor};
use std::net::Shutdown;
use std::ascii::AsciiExt;
use std::mem;
#[cfg(feature = "timeouts")]
use std::time::Duration;

use http::{
    Protocol,
    HttpMessage,
    RequestHead,
    ResponseHead,
    RawStatus,
};
use net::{NetworkStream, NetworkConnector};
use net::{HttpConnector, HttpStream};
use url::Url;
use header::Headers;

use header;
use version;

use solicit::http::Header as Http2Header;
use solicit::http::HttpScheme;
use solicit::http::HttpError as Http2Error;
use solicit::http::transport::TransportStream;
use solicit::http::client::{ClientStream, HttpConnect, HttpConnectError, write_preface};
use solicit::client::SimpleClient;

use httparse;

/// A trait alias representing all types that are both `NetworkStream` and `Clone`.
pub trait CloneableStream: NetworkStream + Clone {}
impl<S: NetworkStream + Clone> CloneableStream for S {}

/// A newtype wrapping any `CloneableStream` in order to provide an implementation of a
/// `TransportSream` trait for all types that are a `CloneableStream`.
#[derive(Clone)]
struct Http2Stream<S: CloneableStream>(S);

impl<S> Write for Http2Stream<S> where S: CloneableStream {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }
    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

impl<S> Read for Http2Stream<S> where S: CloneableStream {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.read(buf)
    }
}

impl<S> TransportStream for Http2Stream<S> where S: CloneableStream {
    fn try_split(&self) -> Result<Http2Stream<S>, io::Error> {
        Ok(self.clone())
    }

    fn close(&mut self) -> Result<(), io::Error> {
        self.0.close(Shutdown::Both)
    }
}

/// A helper struct that implements the `HttpConnect` trait from the `solicit` crate.
///
/// This is used by the `Http2Protocol` when it needs to create a new `SimpleClient`.
struct Http2Connector<S> where S: CloneableStream {
    stream: S,
    scheme: HttpScheme,
    host: String,
}

#[derive(Debug)]
struct Http2ConnectError(io::Error);

impl ::std::fmt::Display for Http2ConnectError {
    fn fmt(&self, fmt: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        write!(fmt, "HTTP/2 connect error: {}", (self as &::std::error::Error).description())
    }
}

impl ::std::error::Error for Http2ConnectError {
    fn description(&self) -> &str {
        self.0.description()
    }

    fn cause(&self) -> Option<&::std::error::Error> {
        self.0.cause()
    }
}

impl HttpConnectError for Http2ConnectError {}

impl From<io::Error> for Http2ConnectError {
    fn from(e: io::Error) -> Http2ConnectError { Http2ConnectError(e) }
}

impl<S> HttpConnect for Http2Connector<S> where S: CloneableStream {
    /// The type of the underlying transport stream that the `HttpConnection`s
    /// produced by this `HttpConnect` implementation will be based on.
    type Stream = Http2Stream<S>;
    /// The type of the error that can be produced by trying to establish the
    /// connection (i.e. calling the `connect` method).
    type Err = Http2ConnectError;

    /// Establishes a network connection that can be used by HTTP/2 connections.
    fn connect(mut self) -> Result<ClientStream<Self::Stream>, Self::Err> {
        try!(write_preface(&mut self.stream));
        Ok(ClientStream(Http2Stream(self.stream), self.scheme, self.host))
    }
}

/// The `Protocol` implementation that provides HTTP/2 messages (i.e. `Http2Message`).
pub struct Http2Protocol<C, S> where C: NetworkConnector<Stream=S> + Send + 'static,
                                 S: NetworkStream + Send + Clone {
    connector: C,
}

impl<C, S> Http2Protocol<C, S> where C: NetworkConnector<Stream=S> + Send + 'static,
                                     S: NetworkStream + Send + Clone {
    /// Create a new `Http2Protocol` that will use the given `NetworkConnector` to establish TCP
    /// connections to the server.
    pub fn with_connector(connector: C) -> Http2Protocol<C, S> {
        Http2Protocol {
            connector: connector,
        }
    }

    /// A private helper method that creates a new `SimpleClient` that will use the given
    /// `NetworkStream` to communicate to the remote host.
    fn new_client(&self, stream: S, host: String, scheme: HttpScheme)
            -> ::Result<SimpleClient<Http2Stream<S>>> {
        Ok(try!(SimpleClient::with_connector(Http2Connector {
            stream: stream,
            scheme: scheme,
            host: host,
        })))
    }
}

impl<C, S> Protocol for Http2Protocol<C, S> where C: NetworkConnector<Stream=S> + Send + 'static,
                                                  S: NetworkStream + Send + Clone {
    fn new_message(&self, host: &str, port: u16, scheme: &str) -> ::Result<Box<HttpMessage>> {
        let stream = try!(self.connector.connect(host, port, scheme)).into();

        let scheme = match scheme {
            "http" => HttpScheme::Http,
            "https" => HttpScheme::Https,
            _ => return Err(From::from(Http2Error::from(
                        io::Error::new(io::ErrorKind::Other, "Invalid scheme")))),
        };
        let client = try!(self.new_client(stream, host.into(), scheme));

        Ok(Box::new(Http2Message::with_client(client)))
    }
}

/// Represents an HTTP/2 request, described by a `RequestHead` and the body of the request.
/// A convenience struct only in use by the `Http2Message`.
#[derive(Clone, Debug)]
struct Http2Request {
    head: RequestHead,
    body: Vec<u8>,
}

/// Represents an HTTP/2 response.
/// A convenience struct only in use by the `Http2Message`.
#[derive(Clone, Debug)]
struct Http2Response {
    body: Cursor<Vec<u8>>,
}

/// The enum tracks the state of the `Http2Message`.
enum MessageState {
    /// State corresponding to no message being set to outgoing yet.
    Idle,
    /// State corresponding to an outgoing message being written out.
    Writing(Http2Request),
    /// State corresponding to an incoming message being read.
    Reading(Http2Response),
}

impl MessageState {
    fn take_request(&mut self) -> Option<Http2Request> {
        match *self {
            MessageState::Idle | MessageState::Reading(_) => return None,
            MessageState::Writing(_) => {},
        }
        let old = mem::replace(self, MessageState::Idle);

        match old {
            // These states are effectively unreachable since we already know the state
            MessageState::Idle | MessageState::Reading(_) => None,
            MessageState::Writing(req) => Some(req),
        }
    }
}

/// An implementation of the `HttpMessage` trait for HTTP/2.
///
/// Relies on the `solicit::http::SimpleClient` for HTTP/2 communication. Adapts both outgoing and
/// incoming messages to the API that `hyper` expects in order to be able to use the message in
/// the `hyper::client` module.
pub struct Http2Message<S> where S: CloneableStream {
    client: SimpleClient<Http2Stream<S>>,
    state: MessageState,
}

impl<S> ::std::fmt::Debug for Http2Message<S> where S: CloneableStream {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> Result<(), ::std::fmt::Error> {
        write!(f, "<Http2Message>")
    }
}

impl<S> Http2Message<S> where S: CloneableStream {
    /// Helper method that creates a new completely fresh `Http2Message`, which will use the given
    /// `SimpleClient` for its HTTP/2 communication.
    fn with_client(client: SimpleClient<Http2Stream<S>>) -> Http2Message<S> {
        Http2Message {
            client: client,
            state: MessageState::Idle,
        }
    }
}

impl<S> Write for Http2Message<S> where S: CloneableStream {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if let MessageState::Writing(ref mut req) = self.state {
            req.body.write(buf)
        } else {
            Err(io::Error::new(io::ErrorKind::Other,
                               "Not in a writable state"))
        }
    }
    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        if let MessageState::Writing(ref mut req) = self.state {
            req.body.flush()
        } else {
            Err(io::Error::new(io::ErrorKind::Other,
                               "Not in a writable state"))
        }
    }
}

impl<S> Read for Http2Message<S> where S: CloneableStream {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if let MessageState::Reading(ref mut res) = self.state {
            res.body.read(buf)
        } else {
            Err(io::Error::new(io::ErrorKind::Other,
                               "Not in a readable state"))
        }
    }
}

/// A helper function that prepares the path of a request by extracting it from the given `Url`.
fn prepare_path(url: Url) -> Vec<u8> {
    let mut uri = url.serialize_path().unwrap();
    if let Some(ref q) = url.query {
        uri.push('?');
        uri.push_str(&q[..]);
    }
    uri.into_bytes()
}

/// A helper function that prepares the headers that should be sent in an HTTP/2 message.
///
/// Adapts the `Headers` into a list of octet string pairs.
fn prepare_headers(mut headers: Headers) -> Vec<Http2Header> {
    if headers.remove::<header::Connection>() {
        warn!("The `Connection` header is not valid for an HTTP/2 connection.");
    }
    let mut http2_headers: Vec<_> = headers.iter().filter_map(|h| {
        if h.is::<header::SetCookie>() {
            None
        } else {
            // HTTP/2 header names MUST be lowercase.
            Some((h.name().to_ascii_lowercase().into_bytes(), h.value_string().into_bytes()))
        }
    }).collect();

    // Now separately add the cookies, as `hyper` considers `Set-Cookie` to be only a single
    // header, even in the face of multiple cookies being set.
    if let Some(set_cookie) = headers.get::<header::SetCookie>() {
        for cookie in set_cookie.iter() {
            http2_headers.push((b"set-cookie".to_vec(), cookie.to_string().into_bytes()));
        }
    }

    http2_headers
}

/// A helper function that prepares the body for sending in an HTTP/2 request.
#[inline]
fn prepare_body(body: Vec<u8>) -> Option<Vec<u8>> {
    if body.len() == 0 {
        None
    } else {
        Some(body)
    }
}

/// Parses a set of HTTP/2 headers into a `hyper::header::Headers` struct.
fn parse_headers(http2_headers: Vec<Http2Header>) -> ::Result<Headers> {
    // Adapt the header name from `Vec<u8>` to `String`, without making any copies.
    let mut headers = Vec::new();
    for (name, value) in http2_headers.into_iter() {
        let name = match String::from_utf8(name) {
            Ok(name) => name,
            Err(_) => return Err(From::from(Http2Error::MalformedResponse)),
        };
        headers.push((name, value));
    }

    let mut raw_headers = Vec::new();
    for &(ref name, ref value) in headers.iter() {
        raw_headers.push(httparse::Header { name: &name, value: &value });
    }

    Headers::from_raw(&raw_headers)
}

/// Parses the response, as returned by `solicit`, into a `ResponseHead` and the full response
/// body.
///
/// Returns them as a two-tuple.
fn parse_response(response: ::solicit::http::Response) -> ::Result<(ResponseHead, Vec<u8>)> {
    let status = try!(response.status_code());
    let headers = try!(parse_headers(response.headers));
    Ok((ResponseHead {
        headers: headers,
        raw_status: RawStatus(status, "".into()),
        version: version::HttpVersion::Http20,
    }, response.body))
}

impl<S> HttpMessage for Http2Message<S> where S: CloneableStream {
    fn set_outgoing(&mut self, head: RequestHead) -> ::Result<RequestHead> {
        match self.state {
            MessageState::Writing(_) | MessageState::Reading(_) => {
                return Err(From::from(Http2Error::from(
                            io::Error::new(io::ErrorKind::Other,
                                           "An outoging has already been set"))));
            },
            MessageState::Idle => {},
        };
        self.state = MessageState::Writing(Http2Request {
            head: head.clone(),
            body: Vec::new(),
        });

        Ok(head)
    }

    fn get_incoming(&mut self) -> ::Result<ResponseHead> {
        // Prepare the request so that it can be passed off to the HTTP/2 client.
        let request = match self.state.take_request() {
            None => {
                return Err(From::from(Http2Error::from(
                            io::Error::new(io::ErrorKind::Other,
                                           "No request in progress"))));
            },
            Some(req) => req,
        };
        let (RequestHead { headers, method, url }, body) = (request.head, request.body);

        let method = method.as_ref().as_bytes();
        let path = prepare_path(url);
        let extra_headers = prepare_headers(headers);
        let body = prepare_body(body);

        // Finally, everything is ready and we issue the request.
        let stream_id = try!(self.client.request(method, &path, &extra_headers, body));

        // Wait for the response
        let resp = try!(self.client.get_response(stream_id));

        // Now that the response is back, adapt it to the structs that hyper expects/provides.
        let (head, body) = try!(parse_response(resp));

        // For now, since `solicit` has already read the full response, we just wrap the body into
        // a `Cursor` to allow for the public interface to support `io::Read`.
        let body = Cursor::new(body);

        // The body is saved so that it can be read out from the message.
        self.state = MessageState::Reading(Http2Response {
            body: body,
        });

        Ok(head)
    }

    #[cfg(feature = "timeouts")]
    #[inline]
    fn set_read_timeout(&self, _dur: Option<Duration>) -> io::Result<()> {
        Ok(())
    }

    #[cfg(feature = "timeouts")]
    #[inline]
    fn set_write_timeout(&self, _dur: Option<Duration>) -> io::Result<()> {
        Ok(())
    }

    #[inline]
    fn close_connection(&mut self) -> ::Result<()> {
        Ok(())
    }
}

/// A convenience method that creates a default `Http2Protocol` that uses a `net::HttpConnector`
/// (which produces an `HttpStream` for the underlying transport layer).
#[inline]
pub fn new_protocol() -> Http2Protocol<HttpConnector, HttpStream> {
    Http2Protocol::with_connector(HttpConnector)
}

#[cfg(test)]
mod tests {
    use super::{Http2Protocol, prepare_headers, parse_headers, parse_response};

    use std::io::{Read};

    use mock::{MockHttp2Connector, MockStream};
    use http::{RequestHead, ResponseHead, Protocol};

    use header::Headers;
    use header;
    use url::Url;
    use method;
    use cookie;
    use version;

    use solicit::http::connection::{HttpFrame, ReceiveFrame};

    /// Tests that the `Http2Message` correctly reads a response with no body.
    #[test]
    fn test_http2_response_no_body() {
        let mut mock_connector = MockHttp2Connector::new();
        mock_connector.new_response_stream(b"200", &Headers::new(), None);
        let protocol = Http2Protocol::with_connector(mock_connector);

        let mut message = protocol.new_message("127.0.0.1", 1337, "http").unwrap();
        message.set_outgoing(RequestHead {
            headers: Headers::new(),
            method: method::Method::Get,
            url: Url::parse("http://127.0.0.1/hello").unwrap(),
        }).unwrap();
        let resp = message.get_incoming().unwrap();

        assert_eq!(resp.raw_status.0, 200);
        let mut body = Vec::new();
        message.read_to_end(&mut body).unwrap();
        assert_eq!(body.len(), 0);
    }

    /// Tests that the `Http2Message` correctly reads a response with a body.
    #[test]
    fn test_http2_response_with_body() {
        let mut mock_connector = MockHttp2Connector::new();
        mock_connector.new_response_stream(b"200", &Headers::new(), Some(vec![1, 2, 3]));
        let protocol = Http2Protocol::with_connector(mock_connector);

        let mut message = protocol.new_message("127.0.0.1", 1337, "http").unwrap();
        message.set_outgoing(RequestHead {
            headers: Headers::new(),
            method: method::Method::Get,
            url: Url::parse("http://127.0.0.1/hello").unwrap(),
        }).unwrap();
        let resp = message.get_incoming().unwrap();

        assert_eq!(resp.raw_status.0, 200);
        let mut body = Vec::new();
        message.read_to_end(&mut body).unwrap();
        assert_eq!(vec![1, 2, 3], body);
    }

    /// Tests that the `Http2Message` correctly reads a response with an empty body.
    #[test]
    fn test_http2_response_empty_body() {
        let mut mock_connector = MockHttp2Connector::new();
        mock_connector.new_response_stream(b"200", &Headers::new(), Some(vec![]));
        let protocol = Http2Protocol::with_connector(mock_connector);

        let mut message = protocol.new_message("127.0.0.1", 1337, "http").unwrap();
        message.set_outgoing(RequestHead {
            headers: Headers::new(),
            method: method::Method::Get,
            url: Url::parse("http://127.0.0.1/hello").unwrap(),
        }).unwrap();
        let resp = message.get_incoming().unwrap();

        assert_eq!(resp.raw_status.0, 200);
        let mut body = Vec::new();
        message.read_to_end(&mut body).unwrap();
        assert_eq!(Vec::<u8>::new(), body);
    }

    /// Tests that the `Http2Message` correctly parses out the headers into the `ResponseHead`.
    #[test]
    fn test_http2_response_headers() {
        let mut mock_connector = MockHttp2Connector::new();
        let mut headers = Headers::new();
        headers.set(header::ContentLength(3));
        headers.set(header::ETag(header::EntityTag::new(true, "tag".into())));
        mock_connector.new_response_stream(b"200", &headers, Some(vec![1, 2, 3]));
        let protocol = Http2Protocol::with_connector(mock_connector);

        let mut message = protocol.new_message("127.0.0.1", 1337, "http").unwrap();
        message.set_outgoing(RequestHead {
            headers: Headers::new(),
            method: method::Method::Get,
            url: Url::parse("http://127.0.0.1/hello").unwrap(),
        }).unwrap();
        let resp = message.get_incoming().unwrap();

        assert_eq!(resp.raw_status.0, 200);
        assert!(resp.headers.has::<header::ContentLength>());
        let &header::ContentLength(len) = resp.headers.get::<header::ContentLength>().unwrap();
        assert_eq!(3, len);
        assert!(resp.headers.has::<header::ETag>());
        let &header::ETag(ref tag) = resp.headers.get::<header::ETag>().unwrap();
        assert_eq!(tag.tag(), "tag");
    }

    /// Tests that an error is returned when the `Http2Message` is not in a readable state.
    #[test]
    fn test_http2_message_not_readable() {
        let mut mock_connector = MockHttp2Connector::new();
        mock_connector.new_response_stream(b"200", &Headers::new(), None);
        let protocol = Http2Protocol::with_connector(mock_connector);

        let mut message = protocol.new_message("127.0.0.1", 1337, "http").unwrap();

        // No outgoing set yet, so nothing can be read at this point.
        assert!(message.read(&mut [0; 5]).is_err());
    }

    /// Tests that an error is returned when the `Http2Message` is not in a writable state.
    #[test]
    fn test_http2_message_not_writable() {
        let mut mock_connector = MockHttp2Connector::new();
        mock_connector.new_response_stream(b"200", &Headers::new(), None);
        let protocol = Http2Protocol::with_connector(mock_connector);

        let mut message = protocol.new_message("127.0.0.1", 1337, "http").unwrap();
        message.set_outgoing(RequestHead {
            headers: Headers::new(),
            method: method::Method::Get,
            url: Url::parse("http://127.0.0.1/hello").unwrap(),
        }).unwrap();
        let _ = message.get_incoming().unwrap();
        // Writes are invalid now
        assert!(message.write(&[1]).is_err());
    }

    /// Asserts that the given stream contains the full expected client preface: the preface bytes,
    /// settings frame, and settings ack frame.
    fn assert_client_preface(server_stream: &mut MockStream) {
        // Skip client preface
        server_stream.read(&mut [0; 24]).unwrap();
        // The first frame are the settings
        assert!(match server_stream.recv_frame().unwrap() {
            HttpFrame::SettingsFrame(_) => true,
            _ => false,
        });
        // Now the ACK to the server's settings.
        assert!(match server_stream.recv_frame().unwrap() {
            HttpFrame::SettingsFrame(_) => true,
            _ => false,
        });
    }

    /// Tests that sending a request with no body works correctly.
    #[test]
    fn test_http2_request_no_body() {
        let mut mock_connector = MockHttp2Connector::new();
        let stream = mock_connector.new_response_stream(b"200", &Headers::new(), Some(vec![]));
        let protocol = Http2Protocol::with_connector(mock_connector);

        let mut message = protocol.new_message("127.0.0.1", 1337, "http").unwrap();
        message.set_outgoing(RequestHead {
            headers: Headers::new(),
            method: method::Method::Get,
            url: Url::parse("http://127.0.0.1/hello").unwrap(),
        }).unwrap();
        let _ = message.get_incoming().unwrap();

        let stream = stream.inner.lock().unwrap();
        assert!(stream.write.len() > 0);
        // The output stream of the client side gets flipped so that we can read the stream from
        // the server's end.
        let mut server_stream = MockStream::with_input(&stream.write);
        assert_client_preface(&mut server_stream);
        let frame = server_stream.recv_frame().unwrap();
        assert!(match frame {
            HttpFrame::HeadersFrame(ref frame) => frame.is_end_of_stream(),
            _ => false,
        });
    }

    /// Tests that sending a request with a body works correctly.
    #[test]
    fn test_http2_request_with_body() {
        let mut mock_connector = MockHttp2Connector::new();
        let stream = mock_connector.new_response_stream(b"200", &Headers::new(), None);
        let protocol = Http2Protocol::with_connector(mock_connector);

        let mut message = protocol.new_message("127.0.0.1", 1337, "http").unwrap();
        message.set_outgoing(RequestHead {
            headers: Headers::new(),
            method: method::Method::Get,
            url: Url::parse("http://127.0.0.1/hello").unwrap(),
        }).unwrap();
        // Write a few things to the request in multiple writes.
        message.write(&[1]).unwrap();
        message.write(&[2, 3]).unwrap();
        let _ = message.get_incoming().unwrap();

        let stream = stream.inner.lock().unwrap();
        assert!(stream.write.len() > 0);
        // The output stream of the client side gets flipped so that we can read the stream from
        // the server's end.
        let mut server_stream = MockStream::with_input(&stream.write);
        assert_client_preface(&mut server_stream);
        let frame = server_stream.recv_frame().unwrap();
        assert!(match frame {
            HttpFrame::HeadersFrame(ref frame) => !frame.is_end_of_stream(),
            _ => false,
        });
        assert!(match server_stream.recv_frame().unwrap() {
            HttpFrame::DataFrame(ref frame) => frame.data == vec![1, 2, 3],
            _ => false,
        });
    }

    /// Tests that headers are correctly prepared when they include a `Set-Cookie` header.
    #[test]
    fn test_http2_prepare_headers_with_set_cookie() {
        let cookies = header::SetCookie(vec![
            cookie::Cookie::new("foo".to_owned(), "bar".to_owned()),
            cookie::Cookie::new("baz".to_owned(), "quux".to_owned())
        ]);
        let mut headers = Headers::new();
        headers.set(cookies);

        let h2headers = prepare_headers(headers);

        assert_eq!(vec![
            (b"set-cookie".to_vec(), b"foo=bar; Path=/".to_vec()),
            (b"set-cookie".to_vec(), b"baz=quux; Path=/".to_vec()),
        ], h2headers);
    }

    /// Tests that headers are correctly prepared when they include a `Cookie` header.
    #[test]
    fn test_http2_prepapre_headers_with_cookie() {
        let cookies = header::Cookie(vec![
            cookie::Cookie::new("foo".to_owned(), "bar".to_owned()),
            cookie::Cookie::new("baz".to_owned(), "quux".to_owned())
        ]);
        let mut headers = Headers::new();
        headers.set(cookies);

        let h2headers = prepare_headers(headers);

        assert_eq!(vec![
            (b"cookie".to_vec(), b"foo=bar; baz=quux".to_vec()),
        ], h2headers);
    }

    /// Tests that HTTP/2 headers are correctly prepared.
    #[test]
    fn test_http2_prepare_headers() {
        let mut headers = Headers::new();
        headers.set(header::ContentLength(3));
        let expected = vec![
            (b"content-length".to_vec(), b"3".to_vec()),
        ];

        assert_eq!(expected, prepare_headers(headers));
    }

    /// Tests that the headers of a response are correctly parsed when they include a `Set-Cookie`
    /// header.
    #[test]
    fn test_http2_parse_headers_with_set_cookie() {
        let h2headers = vec![
            (b"set-cookie".to_vec(), b"foo=bar; Path=/".to_vec()),
            (b"set-cookie".to_vec(), b"baz=quux; Path=/".to_vec()),
        ];
        let expected = header::SetCookie(vec![
            cookie::Cookie::new("foo".to_owned(), "bar".to_owned()),
            cookie::Cookie::new("baz".to_owned(), "quux".to_owned())
        ]);

        let headers = parse_headers(h2headers).unwrap();

        assert!(headers.has::<header::SetCookie>());
        let set_cookie = headers.get::<header::SetCookie>().unwrap();
        assert_eq!(expected, *set_cookie);
    }

    /// Tests that parsing HTTP/2 headers with `Cookie` headers works correctly.
    #[test]
    fn test_http2_parse_headers_with_cookie() {
        let expected = header::Cookie(vec![
            cookie::Cookie::new("foo".to_owned(), "bar".to_owned()),
            cookie::Cookie::new("baz".to_owned(), "quux".to_owned())
        ]);
        // HTTP/2 allows the `Cookie` header to be split into multiple ones to facilitate better
        // compression.
        let h2headers = vec![
            (b"cookie".to_vec(), b"foo=bar".to_vec()),
            (b"cookie".to_vec(), b"baz=quux".to_vec()),
        ];

        let headers = parse_headers(h2headers).unwrap();

        assert!(headers.has::<header::Cookie>());
        assert_eq!(*headers.get::<header::Cookie>().unwrap(), expected);
    }

    /// Tests that the headers of a response are correctly parsed.
    #[test]
    fn test_http2_parse_headers() {
        let h2headers = vec![
            (b":status".to_vec(), b"200".to_vec()),
            (b"content-length".to_vec(), b"3".to_vec()),
        ];

        let headers = parse_headers(h2headers).unwrap();

        assert!(headers.has::<header::ContentLength>());
        let &header::ContentLength(len) = headers.get::<header::ContentLength>().unwrap();
        assert_eq!(3, len);
    }

    /// Tests that if a header name is not a valid utf8 byte sequence, an error is returned.
    #[test]
    fn test_http2_parse_headers_invalid_name() {
        let h2headers = vec![
            (vec![0xfe], vec![]),
        ];

        assert!(parse_headers(h2headers).is_err());
    }

    /// Tests that a response with no pseudo-header for status is considered invalid.
    #[test]
    fn test_http2_parse_response_no_status_code() {
        let response = ::solicit::http::Response {
            body: Vec::new(),
            headers: vec![
                (b"content-length".to_vec(), b"3".to_vec()),
            ],
            stream_id: 1,
        };

        assert!(parse_response(response).is_err());
    }

    /// Tests that an HTTP/2 response gets correctly parsed into a body and response head, when
    /// the body is empty.
    #[test]
    fn test_http2_parse_response_no_body() {
        let response = ::solicit::http::Response {
            body: Vec::new(),
            headers: vec![
                (b":status".to_vec(), b"200".to_vec()),
                (b"content-length".to_vec(), b"0".to_vec()),
            ],
            stream_id: 1,
        };

        let (head, body) = parse_response(response).unwrap();

        assert_eq!(body, vec![]);
        let ResponseHead { headers, raw_status, version } = head;
        assert_eq!(raw_status.0, 200);
        assert_eq!(raw_status.1, "");
        assert!(headers.has::<header::ContentLength>());
        assert_eq!(version, version::HttpVersion::Http20);
    }

    /// Tests that an HTTP/2 response gets correctly parsed into a body and response head, when
    /// the body is not empty.
    #[test]
    fn test_http2_parse_response_with_body() {
        let expected_body = vec![1, 2, 3];
        let response = ::solicit::http::Response {
            body: expected_body.clone(),
            headers: vec![
                (b":status".to_vec(), b"200".to_vec()),
                (b"content-length".to_vec(), b"3".to_vec()),
            ],
            stream_id: 1,
        };

        let (head, body) = parse_response(response).unwrap();

        assert_eq!(body, expected_body);
        let ResponseHead { headers, raw_status, version } = head;
        assert_eq!(raw_status.0, 200);
        assert_eq!(raw_status.1, "");
        assert!(headers.has::<header::ContentLength>());
        assert_eq!(version, version::HttpVersion::Http20);
    }
}
