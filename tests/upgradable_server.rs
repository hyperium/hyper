extern crate bytes;
extern crate hyper;
extern crate futures;
extern crate tokio_core;
extern crate tokio_io;
extern crate tokio_service;

use bytes::BytesMut;
use futures::{Future, Sink, Stream};
use futures::future::{self, Either};
use hyper::{Request, Response, StatusCode};
use hyper::header::{self, Headers, Raw};
use hyper::server::{Http, UpgradableResponse};
use std::ascii::AsciiExt;
use std::io;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::str;
use std::time;
use tokio_core::net::{TcpListener, TcpStream};
use tokio_core::reactor::{Core, Handle};
use tokio_io::codec::{Decoder, Encoder, Framed, FramedParts};
use tokio_service::Service;

struct TestService;

fn detect_handshake(headers: &Headers) -> Option<String> {
    match headers.get::<header::Connection>() {
        None => return None,
        Some(&header::Connection(ref options)) => {
            let upgrade = options.iter().any(|option| {
                match option {
                    &header::ConnectionOption::ConnectionHeader(ref value) if value.as_ref()
                        .eq_ignore_ascii_case("upgrade") => true,
                    _ => false,
                }
            });
            if !upgrade {
                return None;
            }
        }
    }

    match headers.get::<header::Upgrade>() {
        None => return None,
        Some(&header::Upgrade(ref protocols)) => {
            if !protocols.iter()
                .any(|p| p.name == header::ProtocolName::Unregistered("line".into())) {
                return None;
            }
        }
    }

    let echo = match headers.get_raw("x-line-echo").and_then(Raw::one) {
        None => return None,
        Some(echo) => echo,
    };

    str::from_utf8(echo).ok().map(str::to_owned)
}

impl Service for TestService {
    type Request = Request;
    type Response = UpgradableResponse<String>;
    type Error = hyper::Error;
    type Future = Box<Future<Item = Self::Response, Error = Self::Error>>;

    fn call(&self, req: Self::Request) -> Self::Future {
        match detect_handshake(req.headers()) {
            None => {
                Box::new(future::ok(UpgradableResponse::Response(Response::new()
                    .with_status(StatusCode::Ok)
                    .with_header(header::Date(time::UNIX_EPOCH.into()))
                    .with_body("Hello World"))))
            }
            Some(echo) => {
                let res = Response::new()
                    .with_status(StatusCode::SwitchingProtocols)
                    .with_header(header::Date(time::UNIX_EPOCH.into()))
                    .with_header(header::Connection(vec![
                            header::ConnectionOption::ConnectionHeader("Upgrade".parse().unwrap())
                    ]))
                    .with_header(header::Upgrade(vec!["line".parse().unwrap()]));
                Box::new(future::ok(UpgradableResponse::Upgrade(echo, Some(res))))
            }
        }
    }
}

pub struct LineCodec;

impl Encoder for LineCodec {
    type Item = String;
    type Error = io::Error;

    fn encode(&mut self, item: Self::Item, dst: &mut BytesMut) -> Result<(), Self::Error> {
        dst.extend(item.as_bytes());
        dst.extend(b"\n");
        Ok(())
    }
}

impl Decoder for LineCodec {
    type Item = String;
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if let Some(pos) = src.iter().position(|c| *c == b'\n') {
            let line_bytes = src.split_to(pos);
            src.split_to(1);
            str::from_utf8(line_bytes.as_ref())
                .map(|s| Some(s.to_owned()))
                .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))
        } else {
            Ok(None)
        }
    }
}

fn start_server(handle: &Handle) -> SocketAddr {
    let server_handle = handle.clone();

    let server_addr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 0));
    let listener = TcpListener::bind(&server_addr, &handle).expect("listener bind error");
    let server_addr = listener.local_addr().expect("server address retrieval error");

    let server_proto = Http::new();
    let serve = listener.incoming()
        .for_each(move |(tcp, remote_addr)| {
            server_proto.bind_upgradable_connection(&server_handle, tcp, remote_addr, TestService)
                .then(|result| {
                    let maybe_upgrade = result.expect("server http error");
                    let (io, read_buf, echo) = match maybe_upgrade {
                        None => return Either::A(future::ok(())),
                        Some(upgrade) => upgrade,
                    };

                    let parts = FramedParts {
                        inner: io,
                        readbuf: read_buf,
                        writebuf: BytesMut::with_capacity(8192),
                    };
                    let framed = Framed::from_parts(parts, LineCodec);

                    Either::B(framed.send(echo)
                        .then(|result| {
                            let framed = result.expect("server echo error");
                            framed.send("Foo".into())
                        })
                        .then(|result| {
                            let framed = result.expect("server send error");
                            framed.into_future().map_err(|(err, _framed)| err)
                        })
                        .then(|result| {
                            let (maybe_msg, _framed) = result.expect("server receive error");
                            assert_eq!(maybe_msg, Some("Bar".into()));
                            Ok(())
                        }))
                })
        })
        .then(|result| {
            result.expect("server accept error");
            Ok(())
        });
    handle.spawn(serve);

    server_addr
}

#[test]
fn test_http() {
    let mut core = Core::new().expect("core creation error");
    let handle = core.handle();
    let server_addr = start_server(&handle);

    let client = hyper::Client::new(&handle);
    let test = client.get(format!("http://{}", server_addr).parse().expect("uri parse error"))
        .then(|result| {
            let res = result.expect("client http error");
            res.body().concat2()
        })
        .and_then(|body| {
            let body_str = str::from_utf8(body.as_ref()).expect("client body decode error");
            assert_eq!(body_str, "Hello World");
            Ok(())
        });
    core.run(test).expect("client body read error");
}

#[test]
fn test_upgrade() {
    let mut core = Core::new().expect("core creation error");
    let handle = core.handle();
    let server_addr = start_server(&handle);

    let to_server =
        "GET / HTTP/1.1\r\n\
         Host: 127.0.0.1\r\n\
         \r\n\
         GET / HTTP/1.1\r\n\
         Host: 127.0.0.1\r\n\
         Connection: Upgrade\r\n\
         Upgrade: line\r\n\
         X-Line-Echo: Echo\r\n\
         \r\n\
         Bar\n";

    let from_server =
        "HTTP/1.1 200 OK\r\n\
         Date: Thu, 01 Jan 1970 00:00:00 GMT\r\n\
         Transfer-Encoding: chunked\r\n\
         \r\n\
         B\r\n\
         Hello World\r\n\
         0\r\n\
         \r\n\
         HTTP/1.1 101 Switching Protocols\r\n\
         Date: Thu, 01 Jan 1970 00:00:00 GMT\r\n\
         Connection: Upgrade\r\n\
         Upgrade: line\r\n\
         Content-Length: 0\r\n\
         \r\n\
         Echo\n\
         Foo\n";
    let from_server_len = from_server.len();

    let test = TcpStream::connect(&server_addr, &handle)
        .then(move |result| {
            let tcp = result.expect("client connect error");
            tokio_io::io::write_all(tcp, to_server.as_bytes())
        })
        .then(move |result| {
            let (tcp, _msg) = result.expect("client send error");
            let buf = vec![0; from_server_len];
            tokio_io::io::read_exact(tcp, buf)
        })
        .and_then(move |(_tcp, msg)| {
            let msg_str = String::from_utf8(msg).expect("client decode error");
            assert_eq!(msg_str, from_server);
            Ok(())
        });
    core.run(test).expect("client receive error");
}
