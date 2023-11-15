#![deny(warnings)]
#![warn(rust_2018_idioms)]

use std::convert::Infallible;
use std::fmt;
use std::future::Future;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener};
use std::pin::Pin;
use std::thread;
use std::time::Duration;

use http::uri::PathAndQuery;
use http_body_util::{BodyExt, StreamBody};
use hyper::body::Frame;
use hyper::header::HeaderValue;
use hyper::{Method, Request, StatusCode, Uri, Version};

use bytes::Bytes;
use futures_channel::oneshot;
use futures_util::future::{self, FutureExt, TryFuture, TryFutureExt};
use support::TokioIo;
use tokio::net::TcpStream;
mod support;

fn s(buf: &[u8]) -> &str {
    std::str::from_utf8(buf).expect("from_utf8")
}

async fn concat<B>(b: B) -> Result<Bytes, B::Error>
where
    B: hyper::body::Body,
{
    b.collect().await.map(|c| c.to_bytes())
}

async fn tcp_connect(addr: &SocketAddr) -> std::io::Result<TokioIo<TcpStream>> {
    TcpStream::connect(*addr).await.map(TokioIo::new)
}

#[derive(Clone)]
struct HttpInfo {
    remote_addr: SocketAddr,
}

#[derive(Debug)]
enum Error {
    Io(std::io::Error),
    Hyper(hyper::Error),
    AbsoluteUriRequired,
    UnsupportedVersion,
}

impl Error {
    fn is_incomplete_message(&self) -> bool {
        match self {
            Self::Hyper(err) => err.is_incomplete_message(),
            _ => false,
        }
    }

    fn is_parse(&self) -> bool {
        match self {
            Self::Hyper(err) => err.is_parse(),
            _ => false,
        }
    }

    fn is_parse_too_large(&self) -> bool {
        match self {
            Self::Hyper(err) => err.is_parse_too_large(),
            _ => false,
        }
    }

    fn is_parse_status(&self) -> bool {
        match self {
            Self::Hyper(err) => err.is_parse_status(),
            _ => false,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => err.fmt(fmt),
            Self::Hyper(err) => err.fmt(fmt),
            Self::AbsoluteUriRequired => write!(fmt, "client requires absolute-form URIs"),
            Self::UnsupportedVersion => write!(fmt, "request has unsupported HTTP version"),
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

impl From<hyper::Error> for Error {
    fn from(err: hyper::Error) -> Self {
        Self::Hyper(err)
    }
}

macro_rules! test {
    (
        name: $name:ident,
        server:
            expected: $server_expected:expr,
            reply: $server_reply:expr,
        client:
            $(options: {$(
                $c_opt_prop:ident: $c_opt_val:tt,
            )*},)?
            request: {$(
                $c_req_prop:ident: $c_req_val:tt,
            )*},

            response:
                status: $client_status:ident,
                headers: { $($response_header_name:expr => $response_header_val:expr,)* },
                body: $response_body:expr,
    ) => (
        #[test]
        fn $name() {
            let _ = pretty_env_logger::try_init();
            let rt = support::runtime();

            let res = test! {
                INNER;
                name: $name,
                runtime: &rt,
                server:
                    expected: $server_expected,
                    reply: $server_reply,
                client:
                    $(options: {$(
                        $c_opt_prop: $c_opt_val,
                    )*},)?
                    request: {$(
                        $c_req_prop: $c_req_val,
                    )*},
            }.expect("test");


            assert_eq!(res.status(), StatusCode::$client_status);
            $(
                assert_eq!(
                    res
                        .headers()
                        .get($response_header_name)
                        .expect(concat!("response header '", stringify!($response_header_name), "'")),
                    $response_header_val,
                    "response header '{}'",
                    stringify!($response_header_name),
                );
            )*

            let body = rt.block_on(concat(res))
                .expect("body concat wait");

            let expected_res_body = Option::<&[u8]>::from($response_body)
                .unwrap_or_default();
            assert_eq!(body.as_ref(), expected_res_body);
        }
    );
    (
        name: $name:ident,
        server:
            expected: $server_expected:expr,
            reply: $server_reply:expr,
        client:
            request: {$(
                $c_req_prop:ident: $c_req_val:tt,
            )*},

            error: $err:expr,
    ) => (
        #[test]
        fn $name() {
            let _ = pretty_env_logger::try_init();
            let rt = support::runtime();

            let err: Error = test! {
                INNER;
                name: $name,
                runtime: &rt,
                server:
                    expected: $server_expected,
                    reply: $server_reply,
                client:
                    request: {$(
                        $c_req_prop: $c_req_val,
                    )*},
            }.unwrap_err();

            fn infer_closure<F: FnOnce(&Error) -> bool>(f: F) -> F { f }

            let closure = infer_closure($err);
            if !closure(&err) {
                panic!("expected error, unexpected variant: {:?}", err);
            }
        }
    );

    (
        INNER;
        name: $name:ident,
        runtime: $runtime:expr,
        server:
            expected: $server_expected:expr,
            reply: $server_reply:expr,
        client:
            $(options: {$(
                $c_opt_prop:ident: $c_opt_val:tt,
            )*},)?
            request: {$(
                $c_req_prop:ident: $c_req_val:tt,
            )*},
    ) => ({
        let server = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = server.local_addr().expect("local_addr");
        let rt = $runtime;

        #[allow(unused_assignments, unused_mut)]
        let mut body = BodyExt::boxed(http_body_util::Empty::<bytes::Bytes>::new());
        let mut req_builder = Request::builder();
        $(
            test!(@client_request; req_builder, body, addr, $c_req_prop: $c_req_val);
        )*
        let mut req = req_builder
            .body(body)
            .expect("request builder");

        let res = async move {
            // Wrapper around hyper::client::conn::Builder with set_host field to mimic
            // hyper::client::Builder.
            struct Builder {
                inner: hyper::client::conn::http1::Builder,
                set_host: bool,
                http09_responses: bool,
            }

            impl Builder {
                fn new() -> Self {
                    Self {
                        inner: hyper::client::conn::http1::Builder::new(),
                        set_host: true,
                        http09_responses: false,
                    }
                }

                #[allow(unused)]
                fn set_host(&mut self, val: bool) -> &mut Self {
                    self.set_host = val;
                    self
                }

                #[allow(unused)]
                fn http09_responses(&mut self, val: bool) -> &mut Self {
                    self.http09_responses = val;
                    self.inner.http09_responses(val);
                    self
                }
            }

            impl std::ops::Deref for Builder {
                type Target = hyper::client::conn::http1::Builder;

                fn deref(&self) -> &Self::Target {
                    &self.inner
                }
            }

            impl std::ops::DerefMut for Builder {
                fn deref_mut(&mut self) -> &mut Self::Target {
                    &mut self.inner
                }
            }

            #[allow(unused_mut)]
            let mut builder = Builder::new();
            $(builder$(.$c_opt_prop($c_opt_val))*;)?


            if req.version() == Version::HTTP_09 && !builder.http09_responses {
                return Err(Error::UnsupportedVersion);
            }

            if req.version() == Version::HTTP_2 {
                return Err(Error::UnsupportedVersion);
            }

            let host = req.uri().host().ok_or(Error::AbsoluteUriRequired)?;
            let port = req.uri().port_u16().unwrap_or(80);

            let stream = TcpStream::connect(format!("{}:{}", host, port)).await?;

            let extra = HttpInfo {
                remote_addr: stream.peer_addr().unwrap(),
            };

            if builder.set_host {
                let host = req.uri().host().expect("no host in uri");
                let port = req.uri().port_u16().expect("no port in uri");

                let host = format!("{}:{}", host, port);

                req.headers_mut().append("Host", HeaderValue::from_str(&host).unwrap());
            }

            let (mut sender, conn) = builder.handshake(TokioIo::new(stream)).await?;

            tokio::task::spawn(async move {
                if let Err(err) = conn.await {
                    panic!("{}", err);
                }
            });

            let mut builder = Uri::builder();
            if req.method() == Method::CONNECT {
                builder = builder.path_and_query(format!("{}:{}", req.uri().host().unwrap(), req.uri().port_u16().unwrap()));
            } else {
                builder = builder.path_and_query(req.uri().path_and_query().cloned().unwrap_or(PathAndQuery::from_static("/")));
            }
            *req.uri_mut() = builder.build().unwrap();

            let mut resp = sender.send_request(req).await?;

            resp.extensions_mut().insert(extra);
            Ok(resp)
        };

        let (tx, rx) = oneshot::channel();

        let thread = thread::Builder::new()
            .name(format!("tcp-server<{}>", stringify!($name)));
        thread.spawn(move || {
            let mut inc = server.accept().expect("accept").0;
            inc.set_read_timeout(Some(Duration::from_secs(5))).expect("set_read_timeout");
            inc.set_write_timeout(Some(Duration::from_secs(5))).expect("set_write_timeout");
            let expected = format!($server_expected, addr=addr);
            let mut buf = [0; 4096];
            let mut n = 0;
            while n < buf.len() && n < expected.len() {
                n += match inc.read(&mut buf[n..]) {
                    Ok(n) => n,
                    Err(e) => panic!("failed to read request, partially read = {:?}, error: {}", s(&buf[..n]), e),
                };
            }
            assert_eq!(s(&buf[..n]), expected);

            inc.write_all($server_reply.as_ref()).expect("write_all");
            let _ = tx.send(Ok::<_, Error>(()));
        }).expect("thread spawn");

        let rx = rx.expect("thread panicked");

        rt.block_on(future::try_join(res, rx).map_ok(|r| r.0)).map(move |mut resp| {
            // Always check that HttpConnector has set the "extra" info...
            let extra = resp
                .extensions_mut()
                .remove::<HttpInfo>()
                .expect("HttpConnector should set HttpInfo");

            assert_eq!(extra.remote_addr, addr, "HttpInfo should have server addr");

            resp
        })
    });

    (
        @client_request;
        $req_builder:ident,
        $body:ident,
        $addr:ident,
        $c_req_prop:ident: $c_req_val:tt
    ) => ({
        __client_req_prop!($req_builder, $body, $addr, $c_req_prop: $c_req_val)
    });
}

macro_rules! __client_req_prop {
    ($req_builder:ident, $body:ident, $addr:ident, headers: $map:tt) => {{
        __client_req_header!($req_builder, $map)
    }};

    ($req_builder:ident, $body:ident, $addr:ident, method: $method:ident) => {{
        $req_builder = $req_builder.method(Method::$method);
    }};

    ($req_builder:ident, $body:ident, $addr:ident, version: $version:ident) => {{
        $req_builder = $req_builder.version(hyper::Version::$version);
    }};

    ($req_builder:ident, $body:ident, $addr:ident, url: $url:expr) => {{
        $req_builder = $req_builder.uri(format!($url, addr = $addr));
    }};

    ($req_builder:ident, $body:ident, $addr:ident, body: $body_e:expr) => {{
        $body = BodyExt::boxed(http_body_util::Full::from($body_e));
    }};

    ($req_builder:ident, $body:ident, $addr:ident, body_stream: $body_e:expr) => {{
        $body = BodyExt::boxed(StreamBody::new(futures_util::TryStreamExt::map_ok(
            $body_e,
            Frame::data,
        )));
    }};
}

macro_rules! __client_req_header {
    ($req_builder:ident, { $($name:expr => $val:expr,)* }) => {{
        $(
        $req_builder = $req_builder.header($name, $val);
        )*
    }}
}

static REPLY_OK: &str = "HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n";

test! {
    name: client_get,

    server:
        expected: "GET / HTTP/1.1\r\nhost: {addr}\r\n\r\n",
        reply: REPLY_OK,

    client:
        request: {
            method: GET,
            url: "http://{addr}/",
        },
        response:
            status: OK,
            headers: {
                "Content-Length" => "0",
            },
            body: None,
}

test! {
    name: client_get_query,

    server:
        expected: "GET /foo?key=val HTTP/1.1\r\nhost: {addr}\r\n\r\n",
        reply: REPLY_OK,

    client:
        request: {
            method: GET,
            url: "http://{addr}/foo?key=val#dont_send_me",
        },
        response:
            status: OK,
            headers: {
                "Content-Length" => "0",
            },
            body: None,
}

test! {
    name: client_get_req_body_implicitly_empty,

    server:
        expected: "GET / HTTP/1.1\r\nhost: {addr}\r\n\r\n",
        reply: REPLY_OK,

    client:
        request: {
            method: GET,
            url: "http://{addr}/",
            body: "", // not Body::empty
        },
        response:
            status: OK,
            headers: {},
            body: None,
}

test! {
    name: client_get_req_body_chunked,

    server:
        expected: "\
            GET / HTTP/1.1\r\n\
            transfer-encoding: chunked\r\n\
            host: {addr}\r\n\
            \r\n\
            5\r\n\
            hello\r\n\
            0\r\n\r\n\
            ",
        reply: REPLY_OK,

    client:
        request: {
            method: GET,
            url: "http://{addr}/",
            headers: {
                "transfer-encoding" => "chunked",
            },
            body: "hello", // not Body::empty
        },
        response:
            status: OK,
            headers: {},
            body: None,
}

test! {
    name: client_transfer_encoding_repair,

    server:
        expected: "\
            GET / HTTP/1.1\r\n\
            transfer-encoding: foo, chunked\r\n\
            host: {addr}\r\n\
            \r\n\
            5\r\n\
            hello\r\n\
            0\r\n\r\n\
            ",
        reply: REPLY_OK,

    client:
        request: {
            method: GET,
            url: "http://{addr}/",
            headers: {
                "transfer-encoding" => "foo",
            },
            body: "hello", // not Body::empty
        },
        response:
            status: OK,
            headers: {},
            body: None,
}

test! {
    name: client_get_req_body_chunked_http10,

    server:
        expected: "\
            GET / HTTP/1.0\r\n\
            host: {addr}\r\n\
            content-length: 5\r\n\
            \r\n\
            hello\
            ",
        reply: "HTTP/1.0 200 OK\r\ncontent-length: 0\r\n\r\n",

    client:
        request: {
            method: GET,
            url: "http://{addr}/",
            headers: {
                "transfer-encoding" => "chunked",
            },
            version: HTTP_10,
            body: "hello",
        },
        response:
            status: OK,
            headers: {},
            body: None,
}

test! {
    name: client_get_req_body_chunked_with_trailer,

    server:
        expected: "\
            GET / HTTP/1.1\r\n\
            host: {addr}\r\n\
            \r\n\
            ",
        reply: "\
            HTTP/1.1 200 OK\r\n\
            Transfer-Encoding: chunked\r\n\
            \r\n\
            5\r\n\
            hello\r\n\
            0\r\n\
            Trailer: value\r\n\
            \r\n\
            ",

    client:
        request: {
            method: GET,
            url: "http://{addr}/",
        },
        response:
            status: OK,
            headers: {},
            body: &b"hello"[..],
}

test! {
    name: client_get_req_body_chunked_with_multiple_trailers,

    server:
        expected: "\
            GET / HTTP/1.1\r\n\
            host: {addr}\r\n\
            \r\n\
            ",
        reply: "\
            HTTP/1.1 200 OK\r\n\
            Transfer-Encoding: chunked\r\n\
            \r\n\
            5\r\n\
            hello\r\n\
            0\r\n\
            Trailer: value\r\n\
            another-trainer: another-value\r\n\
            \r\n\
            ",

    client:
        request: {
            method: GET,
            url: "http://{addr}/",
        },
        response:
            status: OK,
            headers: {},
            body: &b"hello"[..],
}

test! {
    name: client_get_req_body_sized,

    server:
        expected: "\
            GET / HTTP/1.1\r\n\
            content-length: 5\r\n\
            host: {addr}\r\n\
            \r\n\
            hello\
            ",
        reply: REPLY_OK,

    client:
        request: {
            method: GET,
            url: "http://{addr}/",
            headers: {
                "Content-Length" => "5",
            },
            // use a "stream" (where Body doesn't know length) with a
            // content-length header
            body_stream: (futures_util::stream::once(async {
                Ok::<_, Infallible>(Bytes::from("hello"))
            })),
        },
        response:
            status: OK,
            headers: {},
            body: None,
}

test! {
    name: client_get_req_body_unknown,

    server:
        expected: "\
            GET / HTTP/1.1\r\n\
            host: {addr}\r\n\
            \r\n\
            ",
        reply: REPLY_OK,

    client:
        request: {
            method: GET,
            url: "http://{addr}/",
            // steam means we don't know the content-length,
            // but we're wrapping a non-empty stream.
            //
            // But since the headers cannot tell us, and the method typically
            // doesn't have a body, the body must be ignored.
            body_stream: (futures_util::stream::once(async {
                Ok::<_, Infallible>(Bytes::from("hello"))
            })),
        },
        response:
            status: OK,
            headers: {},
            body: None,
}

test! {
    name: client_get_req_body_unknown_http10,

    server:
        expected: "\
            GET / HTTP/1.0\r\n\
            host: {addr}\r\n\
            \r\n\
            ",
        reply: "HTTP/1.0 200 OK\r\ncontent-length: 0\r\n\r\n",

    client:
        request: {
            method: GET,
            url: "http://{addr}/",
            headers: {
                "transfer-encoding" => "chunked",
            },
            version: HTTP_10,
            // steam means we don't know the content-length,
            // but we're wrapping a non-empty stream.
            //
            // But since the headers cannot tell us, the body must be ignored.
            body_stream: (futures_util::stream::once(async {
                Ok::<_, Infallible>(Bytes::from("hello"))
            })),
        },
        response:
            status: OK,
            headers: {},
            body: None,
}

test! {
    name: client_post_sized,

    server:
        expected: "\
            POST /length HTTP/1.1\r\n\
            content-length: 7\r\n\
            host: {addr}\r\n\
            \r\n\
            foo bar\
            ",
        reply: REPLY_OK,

    client:
        request: {
            method: POST,
            url: "http://{addr}/length",
            headers: {
                "Content-Length" => "7",
            },
            body: "foo bar",
        },
        response:
            status: OK,
            headers: {},
            body: None,
}

test! {
    name: client_post_chunked,

    server:
        expected: "\
            POST /chunks HTTP/1.1\r\n\
            transfer-encoding: chunked\r\n\
            host: {addr}\r\n\
            \r\n\
            B\r\n\
            foo bar baz\r\n\
            0\r\n\r\n\
            ",
        reply: REPLY_OK,

    client:
        request: {
            method: POST,
            url: "http://{addr}/chunks",
            headers: {
                "Transfer-Encoding" => "chunked",
            },
            body: "foo bar baz",
        },
        response:
            status: OK,
            headers: {},
            body: None,
}

test! {
    name: client_post_unknown,

    server:
        expected: "\
            POST /chunks HTTP/1.1\r\n\
            host: {addr}\r\n\
            transfer-encoding: chunked\r\n\
            \r\n\
            B\r\n\
            foo bar baz\r\n\
            0\r\n\r\n\
            ",
        reply: REPLY_OK,

    client:
        request: {
            method: POST,
            url: "http://{addr}/chunks",
            // use a stream to "hide" that the full amount is known
            body_stream: (futures_util::stream::once(async {
                Ok::<_, Infallible>(Bytes::from("foo bar baz"))
            })),
        },
        response:
            status: OK,
            headers: {},
            body: None,
}

test! {
    name: client_post_empty,

    server:
        expected: "\
            POST /empty HTTP/1.1\r\n\
            content-length: 0\r\n\
            host: {addr}\r\n\
            \r\n\
            ",
        reply: REPLY_OK,

    client:
        request: {
            method: POST,
            url: "http://{addr}/empty",
            headers: {
                "Content-Length" => "0",
            },
        },
        response:
            status: OK,
            headers: {},
            body: None,
}

test! {
    name: client_head_ignores_body,

    server:
        expected: "\
            HEAD /head HTTP/1.1\r\n\
            host: {addr}\r\n\
            \r\n\
            ",
        reply: "\
            HTTP/1.1 200 OK\r\n\
            content-Length: 11\r\n\
            \r\n\
            Hello World\
            ",

    client:
        request: {
            method: HEAD,
            url: "http://{addr}/head",
        },
        response:
            status: OK,
            headers: {},
            body: None,
}

test! {
    name: client_response_transfer_encoding_not_chunked,

    server:
        expected: "\
            GET /te-not-chunked HTTP/1.1\r\n\
            host: {addr}\r\n\
            \r\n\
            ",
        reply: "\
            HTTP/1.1 200 OK\r\n\
            transfer-encoding: yolo\r\n\
            \r\n\
            hallo\
            ",

    client:
        request: {
            method: GET,
            url: "http://{addr}/te-not-chunked",
        },
        response:
            status: OK,
            headers: {
                "transfer-encoding" => "yolo",
            },
            body: &b"hallo"[..],
}

test! {
    name: client_pipeline_responses_extra,

    server:
        expected: "\
            GET /pipe HTTP/1.1\r\n\
            host: {addr}\r\n\
            \r\n\
            ",
        reply: "\
            HTTP/1.1 200 OK\r\n\
            Content-Length: 0\r\n\
            \r\n\
            HTTP/1.1 200 OK\r\n\
            Content-Length: 0\r\n\
            \r\n\
            ",

    client:
        request: {
            method: GET,
            url: "http://{addr}/pipe",
        },
        response:
            status: OK,
            headers: {},
            body: None,
}

test! {
    name: client_requires_absolute_uri,

    server:
        expected: "won't get here {addr}",
        reply: "won't reply",

    client:
        request: {
            method: GET,
            url: "/relative-{addr}",
        },
        error: |err| err.to_string() == "client requires absolute-form URIs",
}

test! {
    name: client_error_unexpected_eof,

    server:
        expected: "\
            GET /err HTTP/1.1\r\n\
            host: {addr}\r\n\
            \r\n\
            ",
        reply: "\
            HTTP/1.1 200 OK\r\n\
            ", // unexpected eof before double CRLF

    client:
        request: {
            method: GET,
            url: "http://{addr}/err",
        },
        error: |err| err.is_incomplete_message(),
}

test! {
    name: client_error_parse_version,

    server:
        expected: "\
            GET /err HTTP/1.1\r\n\
            host: {addr}\r\n\
            \r\n\
            ",
        reply: "\
            HEAT/1.1 200 OK\r\n\
            \r\n\
            ",

    client:
        request: {
            method: GET,
            url: "http://{addr}/err",
        },
        // should get a Parse(Version) error
        error: |err| err.is_parse(),

}

test! {
    name: client_error_parse_too_large,

    server:
        expected: "\
            GET /err HTTP/1.1\r\n\
            host: {addr}\r\n\
            \r\n\
            ",
        reply: {
            let long_header = std::iter::repeat("A").take(500_000).collect::<String>();
            format!("\
                HTTP/1.1 200 OK\r\n\
                {}: {}\r\n\
                \r\n\
                ",
                long_header,
                long_header,
            )
        },

    client:
        request: {
            method: GET,
            url: "http://{addr}/err",
        },
        // should get a Parse(TooLarge) error
        error: |err| err.is_parse() && err.is_parse_too_large(),

}

test! {
    name: client_error_parse_status_out_of_range,

    server:
        expected: "\
            GET /err HTTP/1.1\r\n\
            host: {addr}\r\n\
            \r\n\
            ",
        reply: "\
            HTTP/1.1 001 OK\r\n\
            \r\n\
            ",

    client:
        request: {
            method: GET,
            url: "http://{addr}/err",
        },
        // should get a Parse(Status) error
        error: |err| err.is_parse() && err.is_parse_status(),
}

test! {
    name: client_error_parse_status_syntactically_invalid,

    server:
        expected: "\
            GET /err HTTP/1.1\r\n\
            host: {addr}\r\n\
            \r\n\
            ",
        reply: "\
            HTTP/1.1 1 OK\r\n\
            \r\n\
            ",

    client:
        request: {
            method: GET,
            url: "http://{addr}/err",
        },
        // should get a Parse(Status) error
        error: |err| err.is_parse() && err.is_parse_status(),
}

test! {
    name: client_100_continue,

    server:
        expected: "\
            POST /continue HTTP/1.1\r\n\
            content-length: 7\r\n\
            host: {addr}\r\n\
            \r\n\
            foo bar\
            ",
        reply: "\
            HTTP/1.1 100 Continue\r\n\
            \r\n\
            HTTP/1.1 200 OK\r\n\
            Content-Length: 0\r\n\
            \r\n\
            ",

    client:
        request: {
            method: POST,
            url: "http://{addr}/continue",
            headers: {
                "Content-Length" => "7",
            },
            body: "foo bar",
        },
        response:
            status: OK,
            headers: {},
            body: None,
}

test! {
    name: client_connect_method,

    server:
        expected: "\
            CONNECT {addr} HTTP/1.1\r\n\
            host: {addr}\r\n\
            \r\n\
            ",
        reply: "\
            HTTP/1.1 200 OK\r\n\
            \r\n\
            ",

    client:
        request: {
            method: CONNECT,
            url: "{addr}",
        },
        response:
            status: OK,
            headers: {},
            body: None,

}

test! {
    name: client_connect_method_with_absolute_uri,

    server:
        expected: "\
            CONNECT {addr} HTTP/1.1\r\n\
            host: {addr}\r\n\
            \r\n\
            ",
        reply: "\
            HTTP/1.1 200 OK\r\n\
            \r\n\
            ",

    client:
        request: {
            method: CONNECT,
            url: "http://{addr}",
        },
        response:
            status: OK,
            headers: {},
            body: None,

}

test! {
    name: client_set_host_false,

    server:
        // {addr} is here because format! requires it to exist in the string
        expected: "\
            GET /no-host/{addr} HTTP/1.1\r\n\
            \r\n\
            ",
        reply: "\
            HTTP/1.1 200 OK\r\n\
            Content-Length: 0\r\n\
            \r\n\
            ",

    client:
        options: {
            set_host: false,
        },
        request: {
            method: GET,
            url: "http://{addr}/no-host/{addr}",
        },
        response:
            status: OK,
            headers: {},
            body: None,
}

test! {
    name: client_set_http1_title_case_headers,

    server:
        expected: "\
            GET / HTTP/1.1\r\n\
            X-Test-Header: test\r\n\
            Host: {addr}\r\n\
            \r\n\
            ",
        reply: "\
            HTTP/1.1 200 OK\r\n\
            Content-Length: 0\r\n\
            \r\n\
            ",

    client:
        options: {
            title_case_headers: true,
        },
        request: {
            method: GET,
            url: "http://{addr}/",
            headers: {
                "X-Test-Header" => "test",
            },
        },
        response:
            status: OK,
            headers: {},
            body: None,
}

test! {
    name: client_h1_rejects_http2,

    server:
        expected: "won't get here {addr}",
        reply: "won't reply",

    client:
        request: {
            method: GET,
            url: "http://{addr}/",
            version: HTTP_2,
        },
        error: |err| err.to_string() == "request has unsupported HTTP version",
}

test! {
    name: client_always_rejects_http09,

    server:
        expected: "won't get here {addr}",
        reply: "won't reply",

    client:
        request: {
            method: GET,
            url: "http://{addr}/",
            version: HTTP_09,
        },
        error: |err| err.to_string() == "request has unsupported HTTP version",
}

test! {
    name: client_handles_contentlength_values_on_same_line,

    server:
        expected: "GET /foo HTTP/1.1\r\nhost: {addr}\r\n\r\n",
        reply: "\
            HTTP/1.1 200 OK\r\n\
            Content-Length: 3,3\r\n\
            Content-Length: 3,3\r\n\
            \r\n\
            abc\r\n",

    client:
        request: {
            method: GET,
            url: "http://{addr}/foo",
        },
        response:
            status: OK,
            headers: {
            },
            body: &b"abc"[..],
}

test! {
    name: client_allows_http09_when_requested,

    server:
        expected: "\
            GET / HTTP/1.1\r\n\
            host: {addr}\r\n\
            \r\n\
            ",
        reply: "Mmmmh, baguettes.",

    client:
        options: {
            http09_responses: true,
        },
        request: {
            method: GET,
            url: "http://{addr}/",
        },
        response:
            status: OK,
            headers: {},
            body: &b"Mmmmh, baguettes."[..],
}

test! {
    name: client_obs_fold_headers,

    server:
        expected: "\
            GET / HTTP/1.1\r\n\
            host: {addr}\r\n\
            \r\n\
            ",
        reply: "\
            HTTP/1.1 200 OK\r\n\
            Content-Length: 0\r\n\
            Fold: just\r\n some\r\n\t folding\r\n\
            \r\n\
            ",

    client:
        options: {
            allow_obsolete_multiline_headers_in_responses: true,
        },
        request: {
            method: GET,
            url: "http://{addr}/",
        },
        response:
            status: OK,
            headers: {
                "fold" => "just some folding",
            },
            body: None,
}

mod conn {
    use std::error::Error;
    use std::io::{self, Read, Write};
    use std::net::{SocketAddr, TcpListener};
    use std::pin::Pin;
    use std::task::{Context, Poll};
    use std::thread;
    use std::time::Duration;

    use bytes::{Buf, Bytes};
    use futures_channel::{mpsc, oneshot};
    use futures_util::future::{self, poll_fn, FutureExt, TryFutureExt};
    use http_body_util::{BodyExt, Empty, Full, StreamBody};
    use hyper::rt::Timer;
    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
    use tokio::net::{TcpListener as TkTcpListener, TcpStream};

    use hyper::body::{Body, Frame};
    use hyper::client::conn;
    use hyper::upgrade::OnUpgrade;
    use hyper::{Method, Request, Response, StatusCode};

    use super::{concat, s, support, tcp_connect, FutureHyperExt};

    use support::{TokioExecutor, TokioIo, TokioTimer};

    fn setup_logger() {
        let _ = pretty_env_logger::try_init();
    }

    async fn setup_tk_test_server() -> (TkTcpListener, SocketAddr) {
        setup_logger();
        let listener = TkTcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
            .await
            .unwrap();
        let addr = listener.local_addr().unwrap();
        (listener, addr)
    }

    fn setup_std_test_server() -> (TcpListener, SocketAddr) {
        setup_logger();
        let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0))).unwrap();
        let addr = listener.local_addr().unwrap();
        (listener, addr)
    }

    #[tokio::test]
    async fn get() {
        let (listener, addr) = setup_tk_test_server().await;

        let server = async move {
            let mut sock = listener.accept().await.unwrap().0;
            let mut buf = [0; 4096];
            let n = sock.read(&mut buf).await.expect("read 1");

            // Notably:
            // - Just a path, since just a path was set
            // - No host, since no host was set
            let expected = "GET /a HTTP/1.1\r\n\r\n";
            assert_eq!(s(&buf[..n]), expected);

            sock.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
                .await
                .unwrap();
        };

        let client = async move {
            let tcp = tcp_connect(&addr).await.expect("connect");
            let (mut client, conn) = conn::http1::handshake(tcp).await.expect("handshake");

            tokio::task::spawn(async move {
                conn.await.expect("http conn");
            });

            let req = Request::builder()
                .uri("/a")
                .body(Empty::<Bytes>::new())
                .unwrap();
            let mut res = client.send_request(req).await.expect("send_request");
            assert_eq!(res.status(), hyper::StatusCode::OK);
            assert!(res.body_mut().frame().await.is_none());
        };

        future::join(server, client).await;
    }

    #[tokio::test]
    async fn get_custom_reason_phrase() {
        let (listener, addr) = setup_tk_test_server().await;

        let server = async move {
            let mut sock = listener.accept().await.unwrap().0;
            let mut buf = [0; 4096];
            let n = sock.read(&mut buf).await.expect("read 1");

            // Notably:
            // - Just a path, since just a path was set
            // - No host, since no host was set
            let expected = "GET /a HTTP/1.1\r\n\r\n";
            assert_eq!(s(&buf[..n]), expected);

            sock.write_all(b"HTTP/1.1 200 Alright\r\nContent-Length: 0\r\n\r\n")
                .await
                .unwrap();
        };

        let client = async move {
            let tcp = tcp_connect(&addr).await.expect("connect");
            let (mut client, conn) = conn::http1::handshake(tcp).await.expect("handshake");

            tokio::task::spawn(async move {
                conn.await.expect("http conn");
            });

            let req = Request::builder()
                .uri("/a")
                .body(Empty::<Bytes>::new())
                .unwrap();
            let mut res = client.send_request(req).await.expect("send_request");
            assert_eq!(res.status(), hyper::StatusCode::OK);
            assert_eq!(
                res.extensions()
                    .get::<hyper::ext::ReasonPhrase>()
                    .expect("custom reason phrase is present")
                    .as_bytes(),
                &b"Alright"[..]
            );
            assert_eq!(res.headers().len(), 1);
            assert_eq!(
                res.headers().get(http::header::CONTENT_LENGTH).unwrap(),
                "0"
            );
            assert!(res.body_mut().frame().await.is_none());
        };

        future::join(server, client).await;
    }

    #[test]
    fn incoming_content_length() {
        let (server, addr) = setup_std_test_server();
        let rt = support::runtime();

        let (tx1, rx1) = oneshot::channel();

        thread::spawn(move || {
            let mut sock = server.accept().unwrap().0;
            sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            sock.set_write_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut buf = [0; 4096];
            let n = sock.read(&mut buf).expect("read 1");

            let expected = "GET / HTTP/1.1\r\n\r\n";
            assert_eq!(s(&buf[..n]), expected);

            sock.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nhello")
                .unwrap();
            let _ = tx1.send(());
        });

        let tcp = rt.block_on(tcp_connect(&addr)).unwrap();

        let (mut client, conn) = rt.block_on(conn::http1::handshake(tcp)).unwrap();

        rt.spawn(conn.map_err(|e| panic!("conn error: {}", e)).map(|_| ()));

        let req = Request::builder()
            .uri("/")
            .body(Empty::<Bytes>::new())
            .unwrap();
        let res = client.send_request(req).and_then(move |mut res| {
            assert_eq!(res.status(), hyper::StatusCode::OK);
            assert_eq!(res.body().size_hint().exact(), Some(5));
            assert!(!res.body().is_end_stream());
            poll_fn(move |ctx| Pin::new(res.body_mut()).poll_frame(ctx)).map(Option::unwrap)
        });

        let rx = rx1.expect("thread panicked");
        let rx = rx.then(|_| TokioTimer.sleep(Duration::from_millis(200)));
        let chunk = rt.block_on(future::join(res, rx).map(|r| r.0)).unwrap();
        assert_eq!(chunk.data_ref().unwrap().len(), 5);
    }

    #[test]
    fn aborted_body_isnt_completed() {
        let _ = ::pretty_env_logger::try_init();
        let (server, addr) = setup_std_test_server();
        let rt = support::runtime();

        let (tx, rx) = oneshot::channel();
        let server = thread::spawn(move || {
            let mut sock = server.accept().unwrap().0;
            sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            sock.set_write_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let expected = "POST / HTTP/1.1\r\ntransfer-encoding: chunked\r\n\r\n5\r\nhello\r\n";
            let mut buf = vec![0; expected.len()];
            sock.read_exact(&mut buf).expect("read 1");
            assert_eq!(s(&buf), expected);

            let _ = tx.send(());

            assert_eq!(sock.read(&mut buf).expect("read 2"), 0);
        });

        let tcp = rt.block_on(tcp_connect(&addr)).unwrap();

        let (mut client, conn) = rt.block_on(conn::http1::handshake(tcp)).unwrap();

        rt.spawn(conn.map_err(|e| panic!("conn error: {}", e)).map(|_| ()));

        let (mut sender, recv) =
            mpsc::channel::<Result<Frame<Bytes>, Box<dyn Error + Send + Sync>>>(0);

        let sender = thread::spawn(move || {
            sender
                .try_send(Ok(Frame::data("hello".into())))
                .expect("try_send_data");
            support::runtime().block_on(rx).unwrap();

            // Aborts the body in an abnormal fashion.
            let _ = sender.try_send(Err(Box::new(std::io::Error::new(
                io::ErrorKind::Other,
                "body write aborted",
            ))));
        });

        let req = Request::builder()
            .method(Method::POST)
            .uri("/")
            .body(StreamBody::new(recv))
            .unwrap();
        let res = client.send_request(req);
        rt.block_on(res).unwrap_err();

        server.join().expect("server thread panicked");
        sender.join().expect("sender thread panicked");
    }

    #[test]
    fn uri_absolute_form() {
        let (server, addr) = setup_std_test_server();
        let rt = support::runtime();

        let (tx1, rx1) = oneshot::channel();

        thread::spawn(move || {
            let mut sock = server.accept().unwrap().0;
            sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            sock.set_write_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut buf = [0; 4096];
            let n = sock.read(&mut buf).expect("read 1");

            // Notably:
            // - Still no Host header, since it wasn't set
            let expected = "GET http://hyper.local/a HTTP/1.1\r\n\r\n";
            assert_eq!(s(&buf[..n]), expected);

            sock.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
                .unwrap();
            let _ = tx1.send(());
        });

        let tcp = rt.block_on(tcp_connect(&addr)).unwrap();

        let (mut client, conn) = rt.block_on(conn::http1::handshake(tcp)).unwrap();

        rt.spawn(conn.map_err(|e| panic!("conn error: {}", e)).map(|_| ()));

        let req = Request::builder()
            .uri("http://hyper.local/a")
            .body(Empty::<Bytes>::new())
            .unwrap();

        let res = client.send_request(req).and_then(move |res| {
            assert_eq!(res.status(), hyper::StatusCode::OK);
            concat(res)
        });
        let rx = rx1.expect("thread panicked");
        let rx = rx.then(|_| TokioTimer.sleep(Duration::from_millis(200)));
        rt.block_on(future::join(res, rx).map(|r| r.0)).unwrap();
    }

    #[test]
    fn http1_conn_coerces_http2_request() {
        let (server, addr) = setup_std_test_server();
        let rt = support::runtime();

        let (tx1, rx1) = oneshot::channel();

        thread::spawn(move || {
            let mut sock = server.accept().unwrap().0;
            sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            sock.set_write_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut buf = [0; 4096];
            let n = sock.read(&mut buf).expect("read 1");

            // Not HTTP/2, nor panicked
            let expected = "GET /a HTTP/1.1\r\n\r\n";
            assert_eq!(s(&buf[..n]), expected);

            sock.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
                .unwrap();
            let _ = tx1.send(());
        });

        let tcp = rt.block_on(tcp_connect(&addr)).unwrap();

        let (mut client, conn) = rt.block_on(conn::http1::handshake(tcp)).unwrap();

        rt.spawn(conn.map_err(|e| panic!("conn error: {}", e)).map(|_| ()));

        let req = Request::builder()
            .uri("/a")
            .version(hyper::Version::HTTP_2)
            .body(Empty::<Bytes>::new())
            .unwrap();

        let res = client.send_request(req).and_then(move |res| {
            assert_eq!(res.status(), hyper::StatusCode::OK);
            concat(res)
        });
        let rx = rx1.expect("thread panicked");
        let rx = rx.then(|_| TokioTimer.sleep(Duration::from_millis(200)));
        rt.block_on(future::join(res, rx).map(|r| r.0)).unwrap();
    }

    #[test]
    fn pipeline() {
        let (server, addr) = setup_std_test_server();
        let rt = support::runtime();

        let (tx1, rx1) = oneshot::channel();

        thread::spawn(move || {
            let mut sock = server.accept().unwrap().0;
            sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            sock.set_write_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut buf = [0; 4096];
            sock.read(&mut buf).expect("read 1");
            sock.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
                .unwrap();

            let _ = tx1.send(Ok::<_, ()>(()));
        });

        let tcp = rt.block_on(tcp_connect(&addr)).unwrap();

        let (mut client, conn) = rt.block_on(conn::http1::handshake(tcp)).unwrap();

        rt.spawn(conn.map_err(|e| panic!("conn error: {}", e)).map(|_| ()));

        let req = Request::builder()
            .uri("/a")
            .body(Empty::<Bytes>::new())
            .unwrap();
        let res1 = client.send_request(req).and_then(move |res| {
            assert_eq!(res.status(), hyper::StatusCode::OK);
            concat(res)
        });

        // pipelined request will hit NotReady, and thus should return an Error::Cancel
        let req = Request::builder()
            .uri("/b")
            .body(Empty::<Bytes>::new())
            .unwrap();
        let res2 = client.send_request(req).map(|result| {
            let err = result.expect_err("res2");
            assert!(err.is_canceled(), "err not canceled, {:?}", err);
            Ok::<_, ()>(())
        });

        let rx = rx1.expect("thread panicked");
        let rx = rx.then(|_| TokioTimer.sleep(Duration::from_millis(200)));
        rt.block_on(future::join3(res1, res2, rx).map(|r| r.0))
            .unwrap();
    }

    #[test]
    fn upgrade() {
        let (server, addr) = setup_std_test_server();
        let rt = support::runtime();

        let (tx1, rx1) = oneshot::channel();

        thread::spawn(move || {
            let mut sock = server.accept().unwrap().0;
            sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            sock.set_write_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut buf = [0; 4096];
            sock.read(&mut buf).expect("read 1");
            sock.write_all(
                b"\
                HTTP/1.1 101 Switching Protocols\r\n\
                Upgrade: foobar\r\n\
                \r\n\
                foobar=ready\
            ",
            )
            .unwrap();
            let _ = tx1.send(());

            let n = sock.read(&mut buf).expect("read 2");
            assert_eq!(&buf[..n], b"foo=bar");
            sock.write_all(b"bar=foo").expect("write 2");
        });

        let tcp = rt.block_on(tcp_connect(&addr)).unwrap();

        let io = DebugStream {
            tcp,
            shutdown_called: false,
        };

        let (mut client, mut conn) = rt.block_on(conn::http1::handshake(io)).unwrap();

        {
            let until_upgrade = poll_fn(|ctx| conn.poll_without_shutdown(ctx));

            let req = Request::builder()
                .uri("/a")
                .body(Empty::<Bytes>::new())
                .unwrap();
            let res = client.send_request(req).and_then(move |res| {
                assert_eq!(res.status(), hyper::StatusCode::SWITCHING_PROTOCOLS);
                assert_eq!(res.headers()["Upgrade"], "foobar");
                concat(res)
            });

            let rx = rx1.expect("thread panicked");
            let rx = rx.then(|_| TokioTimer.sleep(Duration::from_millis(200)));
            rt.block_on(future::join3(until_upgrade, res, rx).map(|r| r.0))
                .unwrap();

            // should not be ready now
            rt.block_on(poll_fn(|ctx| {
                assert!(client.poll_ready(ctx).is_pending());
                Poll::Ready(Ok::<_, ()>(()))
            }))
            .unwrap();
        }

        let parts = conn.into_parts();
        let io = parts.io;
        let buf = parts.read_buf;

        assert_eq!(buf, b"foobar=ready"[..]);
        assert!(!io.shutdown_called, "upgrade shouldn't shutdown AsyncWrite");
        rt.block_on(poll_fn(|ctx| {
            let ready = client.poll_ready(ctx);
            assert!(matches!(ready, Poll::Ready(Err(_))));
            ready
        }))
        .unwrap_err();

        let mut io = io.tcp.inner();
        let mut vec = vec![];
        rt.block_on(io.write_all(b"foo=bar")).unwrap();
        rt.block_on(io.read_to_end(&mut vec)).unwrap();
        assert_eq!(vec, b"bar=foo");
    }

    #[test]
    fn connect_method() {
        let (server, addr) = setup_std_test_server();
        let rt = support::runtime();

        let (tx1, rx1) = oneshot::channel();

        thread::spawn(move || {
            let mut sock = server.accept().unwrap().0;
            sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            sock.set_write_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut buf = [0; 4096];
            sock.read(&mut buf).expect("read 1");
            sock.write_all(
                b"\
                HTTP/1.1 200 OK\r\n\
                \r\n\
                foobar=ready\
            ",
            )
            .unwrap();
            let _ = tx1.send(Ok::<_, ()>(()));

            let n = sock.read(&mut buf).expect("read 2");
            assert_eq!(&buf[..n], b"foo=bar", "sock read 2 bytes");
            sock.write_all(b"bar=foo").expect("write 2");
        });

        let tcp = rt.block_on(tcp_connect(&addr)).unwrap();

        let io = DebugStream {
            tcp,
            shutdown_called: false,
        };

        let (mut client, mut conn) = rt.block_on(conn::http1::handshake(io)).unwrap();

        {
            let until_tunneled = poll_fn(|ctx| conn.poll_without_shutdown(ctx));

            let req = Request::builder()
                .method("CONNECT")
                .uri(addr.to_string())
                .body(Empty::<Bytes>::new())
                .unwrap();
            let res = client
                .send_request(req)
                .and_then(move |res| {
                    assert_eq!(res.status(), hyper::StatusCode::OK);
                    concat(res)
                })
                .map_ok(|body| {
                    assert_eq!(body.as_ref(), b"");
                });

            let rx = rx1.expect("thread panicked");
            let rx = rx.then(|_| TokioTimer.sleep(Duration::from_millis(200)));
            rt.block_on(future::join3(until_tunneled, res, rx).map(|r| r.0))
                .unwrap();

            // should not be ready now
            rt.block_on(poll_fn(|ctx| {
                assert!(client.poll_ready(ctx).is_pending());
                Poll::Ready(Ok::<_, ()>(()))
            }))
            .unwrap();
        }

        let parts = conn.into_parts();
        let io = parts.io;
        let buf = parts.read_buf;

        assert_eq!(buf, b"foobar=ready"[..]);
        assert!(!io.shutdown_called, "tunnel shouldn't shutdown AsyncWrite");

        rt.block_on(poll_fn(|ctx| {
            let ready = client.poll_ready(ctx);
            assert!(matches!(ready, Poll::Ready(Err(_))));
            ready
        }))
        .unwrap_err();

        let mut io = io.tcp.inner();
        let mut vec = vec![];
        rt.block_on(io.write_all(b"foo=bar")).unwrap();
        rt.block_on(io.read_to_end(&mut vec)).unwrap();
        assert_eq!(vec, b"bar=foo");
    }

    #[tokio::test]
    async fn http2_detect_conn_eof() {
        use futures_util::future;

        let (listener, addr) = setup_tk_test_server().await;

        let (shdn_tx, mut shdn_rx) = tokio::sync::watch::channel(false);
        tokio::task::spawn(async move {
            use hyper::server::conn::http2;
            use hyper::service::service_fn;

            loop {
                tokio::select! {
                    res = listener.accept() => {
                        let (stream, _) = res.unwrap();
                        let stream = TokioIo::new(stream);

                        let service = service_fn(|_:Request<hyper::body::Incoming>| future::ok::<_, hyper::Error>(Response::new(Empty::<Bytes>::new())));

                        let mut shdn_rx = shdn_rx.clone();
                        tokio::task::spawn(async move {
                            let mut conn = http2::Builder::new(TokioExecutor)
                                .serve_connection(stream, service);

                            tokio::select! {
                                res = &mut conn => {
                                    res.unwrap();
                                }
                                _ = shdn_rx.changed() => {
                                    Pin::new(&mut conn).graceful_shutdown();
                                    conn.await.unwrap();
                                }
                            }
                        });
                    }
                    _ = shdn_rx.changed() => {
                        break;
                    }
                }
            }
        });

        let io = tcp_connect(&addr).await.expect("tcp connect");
        let (mut client, conn) = conn::http2::Builder::new(TokioExecutor)
            .handshake(io)
            .await
            .expect("http handshake");

        tokio::task::spawn(async move {
            conn.await.expect("client conn");
        });

        // Sanity check that client is ready
        future::poll_fn(|ctx| client.poll_ready(ctx))
            .await
            .expect("client poll ready sanity");

        let req = Request::builder()
            .uri(format!("http://{}/", addr))
            .body(Empty::<Bytes>::new())
            .expect("request builder");

        client.send_request(req).await.expect("req1 send");

        // Sanity check that client is STILL ready
        future::poll_fn(|ctx| client.poll_ready(ctx))
            .await
            .expect("client poll ready after");

        // Trigger the server shutdown...
        let _ = shdn_tx.send(true);

        // Allow time for graceful shutdown roundtrips...
        TokioTimer.sleep(Duration::from_millis(100)).await;

        // After graceful shutdown roundtrips, the client should be closed...
        future::poll_fn(|ctx| client.poll_ready(ctx))
            .await
            .expect_err("client should be closed");
    }

    #[tokio::test]
    async fn http2_keep_alive_detects_unresponsive_server() {
        let (listener, addr) = setup_tk_test_server().await;

        // spawn a server that reads but doesn't write
        tokio::spawn(async move {
            let mut sock = listener.accept().await.unwrap().0;
            let mut buf = [0u8; 1024];
            loop {
                let n = sock.read(&mut buf).await.expect("server read");
                if n == 0 {
                    // server closed, lets go!
                    break;
                }
            }
        });

        let io = tcp_connect(&addr).await.expect("tcp connect");
        let (_client, conn) = conn::http2::Builder::new(TokioExecutor)
            .timer(TokioTimer)
            .keep_alive_interval(Duration::from_secs(1))
            .keep_alive_timeout(Duration::from_secs(1))
            // enable while idle since we aren't sending requests
            .keep_alive_while_idle(true)
            .handshake::<_, hyper::body::Incoming>(io)
            .await
            .expect("http handshake");

        conn.await.expect_err("conn should time out");
    }

    #[tokio::test]
    async fn http2_keep_alive_not_while_idle() {
        // This tests that not setting `http2_keep_alive_while_idle(true)`
        // will use the default behavior which will NOT detect the server
        // is unresponsive while no streams are active.

        let (listener, addr) = setup_tk_test_server().await;

        // spawn a server that reads but doesn't write
        tokio::spawn(async move {
            let sock = listener.accept().await.unwrap().0;
            drain_til_eof(sock).await.expect("server read");
        });

        let io = tcp_connect(&addr).await.expect("tcp connect");
        let (mut client, conn) = conn::http2::Builder::new(TokioExecutor)
            .timer(TokioTimer)
            .keep_alive_interval(Duration::from_secs(1))
            .keep_alive_timeout(Duration::from_secs(1))
            .handshake::<_, hyper::body::Incoming>(io)
            .await
            .expect("http handshake");

        tokio::spawn(async move {
            conn.await.expect("client conn shouldn't error");
        });

        // sleep longer than keepalive would trigger
        TokioTimer.sleep(Duration::from_secs(4)).await;

        future::poll_fn(|ctx| client.poll_ready(ctx))
            .await
            .expect("client should be open");
    }

    #[tokio::test]
    async fn http2_keep_alive_closes_open_streams() {
        let (listener, addr) = setup_tk_test_server().await;

        // spawn a server that reads but doesn't write
        tokio::spawn(async move {
            let sock = listener.accept().await.unwrap().0;
            drain_til_eof(sock).await.expect("server read");
        });

        let io = tcp_connect(&addr).await.expect("tcp connect");
        let (mut client, conn) = conn::http2::Builder::new(TokioExecutor)
            .timer(TokioTimer)
            .keep_alive_interval(Duration::from_secs(1))
            .keep_alive_timeout(Duration::from_secs(1))
            .handshake(io)
            .await
            .expect("http handshake");

        tokio::spawn(async move {
            let err = conn.await.expect_err("client conn should timeout");
            assert!(err.is_timeout());
        });

        let req = http::Request::new(Empty::<Bytes>::new());
        let err = client
            .send_request(req)
            .await
            .expect_err("request should timeout");
        assert!(err.is_timeout());

        let err = future::poll_fn(|ctx| client.poll_ready(ctx))
            .await
            .expect_err("client should be closed");
        assert!(
            err.is_closed(),
            "poll_ready error should be closed: {:?}",
            err
        );
    }

    #[tokio::test]
    async fn http2_keep_alive_with_responsive_server() {
        // Test that a responsive server works just when client keep
        // alive is enabled
        use hyper::service::service_fn;

        let (listener, addr) = setup_tk_test_server().await;

        // Spawn an HTTP2 server that reads the whole body and responds
        tokio::spawn(async move {
            let sock = TokioIo::new(listener.accept().await.unwrap().0);
            hyper::server::conn::http2::Builder::new(TokioExecutor)
                .timer(TokioTimer)
                .serve_connection(
                    sock,
                    service_fn(|req| async move {
                        tokio::spawn(async move {
                            let _ = concat(req.into_body())
                                .await
                                .expect("server req body aggregate");
                        });
                        Ok::<_, hyper::Error>(http::Response::new(Empty::<Bytes>::new()))
                    }),
                )
                .await
                .expect("serve_connection");
        });

        let io = tcp_connect(&addr).await.expect("tcp connect");
        let (mut client, conn) = conn::http2::Builder::new(TokioExecutor)
            .timer(TokioTimer)
            .keep_alive_interval(Duration::from_secs(1))
            .keep_alive_timeout(Duration::from_secs(1))
            .handshake(io)
            .await
            .expect("http handshake");

        tokio::spawn(async move {
            conn.await.expect("client conn shouldn't error");
        });

        // Use a channel to keep request stream open
        let (_tx, recv) = mpsc::channel::<Result<Frame<Bytes>, Box<dyn Error + Send + Sync>>>(0);
        let req = http::Request::new(StreamBody::new(recv));

        let _resp = client.send_request(req).await.expect("send_request");

        // sleep longer than keepalive would trigger
        TokioTimer.sleep(Duration::from_secs(4)).await;

        future::poll_fn(|ctx| client.poll_ready(ctx))
            .await
            .expect("client should be open");
    }

    #[tokio::test]
    async fn http2_responds_before_consuming_request_body() {
        // Test that a early-response from server works correctly (request body wasn't fully consumed).
        // https://github.com/hyperium/hyper/issues/2872
        use hyper::service::service_fn;

        let _ = pretty_env_logger::try_init();

        let (listener, addr) = setup_tk_test_server().await;

        // Spawn an HTTP2 server that responds before reading the whole request body.
        // It's normal case to decline the request due to headers or size of the body.
        tokio::spawn(async move {
            let sock = TokioIo::new(listener.accept().await.unwrap().0);
            hyper::server::conn::http2::Builder::new(TokioExecutor)
                .timer(TokioTimer)
                .serve_connection(
                    sock,
                    service_fn(|_req| async move {
                        Ok::<_, hyper::Error>(Response::new(Full::new(Bytes::from(
                            "No bread for you!",
                        ))))
                    }),
                )
                .await
                .expect("serve_connection");
        });

        let io = tcp_connect(&addr).await.expect("tcp connect");
        let (mut client, conn) = conn::http2::Builder::new(TokioExecutor)
            .timer(TokioTimer)
            .handshake(io)
            .await
            .expect("http handshake");

        tokio::spawn(async move {
            conn.await.expect("client conn shouldn't error");
        });

        // Use a channel to keep request stream open
        let (_tx, recv) = mpsc::channel::<Result<Frame<Bytes>, Box<dyn Error + Send + Sync>>>(0);
        let req = Request::post("/a").body(StreamBody::new(recv)).unwrap();
        let resp = client.send_request(req).await.expect("send_request");
        assert!(resp.status().is_success());

        let mut body = String::new();
        concat(resp.into_body())
            .await
            .unwrap()
            .reader()
            .read_to_string(&mut body)
            .unwrap();

        assert_eq!(&body, "No bread for you!");
    }

    #[tokio::test]
    async fn h2_connect() {
        let (listener, addr) = setup_tk_test_server().await;

        // Spawn an HTTP2 server that asks for bread and responds with baguette.
        tokio::spawn(async move {
            let sock = listener.accept().await.unwrap().0;
            let mut h2 = h2::server::handshake(sock).await.unwrap();

            let (req, mut respond) = h2.accept().await.unwrap().unwrap();
            tokio::spawn(async move {
                poll_fn(|cx| h2.poll_closed(cx)).await.unwrap();
            });
            assert_eq!(req.method(), Method::CONNECT);

            let mut body = req.into_body();

            let mut send_stream = respond.send_response(Response::new(()), false).unwrap();

            send_stream.send_data("Bread?".into(), true).unwrap();

            let bytes = body.data().await.unwrap().unwrap();
            assert_eq!(&bytes[..], b"Baguette!");
            let _ = body.flow_control().release_capacity(bytes.len());

            assert!(body.data().await.is_none());
        });

        let io = tcp_connect(&addr).await.expect("tcp connect");
        let (mut client, conn) = conn::http2::Builder::new(TokioExecutor)
            .handshake(io)
            .await
            .expect("http handshake");

        tokio::spawn(async move {
            conn.await.expect("client conn shouldn't error");
        });

        let req = Request::connect("localhost")
            .body(Empty::<Bytes>::new())
            .unwrap();
        let res = client.send_request(req).await.expect("send_request");
        assert_eq!(res.status(), StatusCode::OK);

        let mut upgraded = TokioIo::new(hyper::upgrade::on(res).await.unwrap());

        let mut vec = vec![];
        upgraded.read_to_end(&mut vec).await.unwrap();
        assert_eq!(s(&vec), "Bread?");

        upgraded.write_all(b"Baguette!").await.unwrap();

        upgraded.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn h2_connect_rejected() {
        let (listener, addr) = setup_tk_test_server().await;
        let (done_tx, done_rx) = oneshot::channel();

        tokio::spawn(async move {
            let sock = listener.accept().await.unwrap().0;
            let mut h2 = h2::server::handshake(sock).await.unwrap();

            let (req, mut respond) = h2.accept().await.unwrap().unwrap();
            tokio::spawn(async move {
                poll_fn(|cx| h2.poll_closed(cx)).await.unwrap();
            });
            assert_eq!(req.method(), Method::CONNECT);

            let res = Response::builder().status(400).body(()).unwrap();
            let mut send_stream = respond.send_response(res, false).unwrap();
            send_stream
                .send_data("No bread for you!".into(), true)
                .unwrap();
            done_rx.await.unwrap();
        });

        let io = tcp_connect(&addr).await.expect("tcp connect");
        let (mut client, conn) = conn::http2::Builder::new(TokioExecutor)
            .handshake::<_, Empty<Bytes>>(io)
            .await
            .expect("http handshake");

        tokio::spawn(async move {
            conn.await.expect("client conn shouldn't error");
        });

        let req = Request::connect("localhost").body(Empty::new()).unwrap();
        let res = client.send_request(req).await.expect("send_request");
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
        assert!(res.extensions().get::<OnUpgrade>().is_none());

        let mut body = String::new();
        concat(res.into_body())
            .await
            .unwrap()
            .reader()
            .read_to_string(&mut body)
            .unwrap();
        assert_eq!(body, "No bread for you!");

        done_tx.send(()).unwrap();
    }

    #[tokio::test]
    async fn test_body_panics() {
        let (listener, addr) = setup_tk_test_server().await;

        // spawn a server that reads but doesn't write
        tokio::spawn(async move {
            let sock = listener.accept().await.unwrap().0;
            drain_til_eof(sock).await.expect("server read");
        });

        let io = tcp_connect(&addr).await.expect("tcp connect");

        let (mut client, conn) = conn::http1::Builder::new()
            .handshake(io)
            .await
            .expect("handshake");

        tokio::spawn(async move {
            conn.await.expect("client conn shouldn't error");
        });

        let req = Request::post("/a")
            .body(http_body_util::BodyExt::map_frame::<_, bytes::Bytes>(
                http_body_util::Full::<bytes::Bytes>::from("baguette"),
                |_| panic!("oopsie"),
            ))
            .unwrap();

        let error = client.send_request(req).await.unwrap_err();

        assert!(error.is_user());
    }

    async fn drain_til_eof<T: tokio::io::AsyncRead + Unpin>(mut sock: T) -> io::Result<()> {
        let mut buf = [0u8; 1024];
        loop {
            let n = sock.read(&mut buf).await?;
            if n == 0 {
                // socket closed, lets go!
                return Ok(());
            }
        }
    }

    struct DebugStream {
        tcp: TokioIo<TcpStream>,
        shutdown_called: bool,
    }

    impl hyper::rt::Write for DebugStream {
        fn poll_shutdown(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
        ) -> Poll<Result<(), io::Error>> {
            self.shutdown_called = true;
            Pin::new(&mut self.tcp).poll_shutdown(cx)
        }

        fn poll_flush(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
        ) -> Poll<Result<(), io::Error>> {
            Pin::new(&mut self.tcp).poll_flush(cx)
        }

        fn poll_write(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<Result<usize, io::Error>> {
            Pin::new(&mut self.tcp).poll_write(cx, buf)
        }
    }

    impl hyper::rt::Read for DebugStream {
        fn poll_read(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: hyper::rt::ReadBufCursor<'_>,
        ) -> Poll<io::Result<()>> {
            Pin::new(&mut self.tcp).poll_read(cx, buf)
        }
    }
}

trait FutureHyperExt: TryFuture {
    fn expect(self, msg: &'static str) -> Pin<Box<dyn Future<Output = Self::Ok>>>;
}

impl<F> FutureHyperExt for F
where
    F: TryFuture + 'static,
    F::Error: std::fmt::Debug,
{
    fn expect(self, msg: &'static str) -> Pin<Box<dyn Future<Output = Self::Ok>>> {
        Box::pin(
            self.inspect_err(move |e| panic!("expect: {}; error={:?}", msg, e))
                .map(Result::unwrap),
        )
    }
}
