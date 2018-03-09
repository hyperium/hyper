use std::borrow::Cow;
use std::fmt::{self, Write};

use httparse;
use bytes::{BytesMut, Bytes};

use header::{self, Headers, ContentLength, TransferEncoding};
use proto::{Decode, MessageHead, RawStatus, Http1Transaction, ParseResult,
           RequestLine, RequestHead};
use proto::h1::{Encoder, Decoder, date};
use method::Method;
use status::StatusCode;
use version::HttpVersion::{Http10, Http11};

const MAX_HEADERS: usize = 100;
const AVERAGE_HEADER_SIZE: usize = 30; // totally scientific

// There are 2 main roles, Client and Server.
//
// There is 1 modifier, OnUpgrade, which can wrap Client and Server,
// to signal that HTTP upgrades are not supported.

pub struct Client<T>(T);

pub struct Server<T>(T);

impl<T> Http1Transaction for Server<T>
where
    T: OnUpgrade,
{
    type Incoming = RequestLine;
    type Outgoing = StatusCode;

    fn parse(buf: &mut BytesMut) -> ParseResult<RequestLine> {
        if buf.len() == 0 {
            return Ok(None);
        }
        let mut headers_indices = [HeaderIndices {
            name: (0, 0),
            value: (0, 0)
        }; MAX_HEADERS];
        let (len, method, path, version, headers_len) = {
            let mut headers = [httparse::EMPTY_HEADER; MAX_HEADERS];
            trace!("Request.parse([Header; {}], [u8; {}])", headers.len(), buf.len());
            let mut req = httparse::Request::new(&mut headers);
            match try!(req.parse(&buf)) {
                httparse::Status::Complete(len) => {
                    trace!("Request.parse Complete({})", len);
                    let method = try!(req.method.unwrap().parse());
                    let path = req.path.unwrap();
                    let bytes_ptr = buf.as_ref().as_ptr() as usize;
                    let path_start = path.as_ptr() as usize - bytes_ptr;
                    let path_end = path_start + path.len();
                    let path = (path_start, path_end);
                    let version = if req.version.unwrap() == 1 { Http11 } else { Http10 };

                    record_header_indices(buf.as_ref(), &req.headers, &mut headers_indices);
                    let headers_len = req.headers.len();
                    (len, method, path, version, headers_len)
                }
                httparse::Status::Partial => return Ok(None),
            }
        };

        let mut headers = Headers::with_capacity(headers_len);
        let slice = buf.split_to(len).freeze();
        let path = slice.slice(path.0, path.1);
        // path was found to be utf8 by httparse
        let path = try!(unsafe { ::uri::from_utf8_unchecked(path) });
        let subject = RequestLine(
            method,
            path,
        );

        headers.extend(HeadersAsBytesIter {
            headers: headers_indices[..headers_len].iter(),
            slice: slice,
        });

        Ok(Some((MessageHead {
            version: version,
            subject: subject,
            headers: headers,
        }, len)))
    }

    fn decoder(head: &MessageHead<Self::Incoming>, method: &mut Option<Method>) -> ::Result<Decode> {
        use ::header;

        *method = Some(head.subject.0.clone());

        // According to https://tools.ietf.org/html/rfc7230#section-3.3.3
        // 1. (irrelevant to Request)
        // 2. (irrelevant to Request)
        // 3. Transfer-Encoding: chunked has a chunked body.
        // 4. If multiple differing Content-Length headers or invalid, close connection.
        // 5. Content-Length header has a sized body.
        // 6. Length 0.
        // 7. (irrelevant to Request)

        if let Some(&header::TransferEncoding(ref encodings)) = head.headers.get() {
            // https://tools.ietf.org/html/rfc7230#section-3.3.3
            // If Transfer-Encoding header is present, and 'chunked' is
            // not the final encoding, and this is a Request, then it is
            // mal-formed. A server should respond with 400 Bad Request.
            if head.version == Http10 {
                debug!("HTTP/1.0 has Transfer-Encoding header");
                Err(::Error::Header)
            } else if encodings.last() == Some(&header::Encoding::Chunked) {
                Ok(Decode::Normal(Decoder::chunked()))
            } else {
                debug!("request with transfer-encoding header, but not chunked, bad request");
                Err(::Error::Header)
            }
        } else if let Some(&header::ContentLength(len)) = head.headers.get() {
            Ok(Decode::Normal(Decoder::length(len)))
        } else if head.headers.has::<header::ContentLength>() {
            debug!("illegal Content-Length: {:?}", head.headers.get_raw("Content-Length"));
            Err(::Error::Header)
        } else {
            Ok(Decode::Normal(Decoder::length(0)))
        }
    }


    fn encode(mut head: MessageHead<Self::Outgoing>, has_body: bool, method: &mut Option<Method>, dst: &mut Vec<u8>) -> ::Result<Encoder> {
        trace!("Server::encode has_body={}, method={:?}", has_body, method);

        // hyper currently doesn't support returning 1xx status codes as a Response
        // This is because Service only allows returning a single Response, and
        // so if you try to reply with a e.g. 100 Continue, you have no way of
        // replying with the latter status code response.
        let ret = if ::StatusCode::SwitchingProtocols == head.subject {
            T::on_encode_upgrade(&mut head)
                .map(|_| {
                    let mut enc = Server::set_length(&mut head, has_body, method.as_ref());
                    enc.set_last();
                    enc
                })
        } else if head.subject.is_informational() {
            error!("response with 1xx status code not supported");
            head = MessageHead::default();
            head.subject = ::StatusCode::InternalServerError;
            head.headers.set(ContentLength(0));
            Err(::Error::Status)
        } else {
            Ok(Server::set_length(&mut head, has_body, method.as_ref()))
        };


        let init_cap = 30 + head.headers.len() * AVERAGE_HEADER_SIZE;
        dst.reserve(init_cap);
        if head.version == ::HttpVersion::Http11 && head.subject == ::StatusCode::Ok {
            extend(dst, b"HTTP/1.1 200 OK\r\n");
            let _ = write!(FastWrite(dst), "{}", head.headers);
        } else {
            let _ = write!(FastWrite(dst), "{} {}\r\n{}", head.version, head.subject, head.headers);
        }
        // using http::h1::date is quite a lot faster than generating a unique Date header each time
        // like req/s goes up about 10%
        if !head.headers.has::<header::Date>() {
            dst.reserve(date::DATE_VALUE_LENGTH + 8);
            extend(dst, b"Date: ");
            date::extend(dst);
            extend(dst, b"\r\n");
        }
        extend(dst, b"\r\n");

        ret
    }

    fn on_error(err: &::Error) -> Option<MessageHead<Self::Outgoing>> {
        let status = match err {
            &::Error::Method |
            &::Error::Version |
            &::Error::Header |
            &::Error::Uri(_) => {
                StatusCode::BadRequest
            },
            &::Error::TooLarge => {
                StatusCode::RequestHeaderFieldsTooLarge
            }
            _ => return None,
        };

        debug!("sending automatic response ({}) for parse error", status);
        let mut msg = MessageHead::default();
        msg.subject = status;
        Some(msg)
    }

    fn should_error_on_parse_eof() -> bool {
        false
    }

    fn should_read_first() -> bool {
        true
    }
}

impl Server<()> {
    fn set_length(head: &mut MessageHead<StatusCode>, has_body: bool, method: Option<&Method>) -> Encoder {
        // these are here thanks to borrowck
        // `if method == Some(&Method::Get)` says the RHS doesn't live long enough
        const HEAD: Option<&'static Method> = Some(&Method::Head);
        const CONNECT: Option<&'static Method> = Some(&Method::Connect);

        let can_have_body = {
            if method == HEAD {
                false
            } else if method == CONNECT && head.subject.is_success() {
                false
            } else {
                match head.subject {
                    // TODO: support for 1xx codes needs improvement everywhere
                    // would be 100...199 => false
                    StatusCode::NoContent |
                    StatusCode::NotModified => false,
                    _ => true,
                }
            }
        };

        if has_body && can_have_body {
            set_length(&mut head.headers, head.version == Http11)
        } else {
            head.headers.remove::<TransferEncoding>();
            if can_have_body {
                head.headers.set(ContentLength(0));
            }
            Encoder::length(0)
        }
    }
}

impl<T> Http1Transaction for Client<T>
where
    T: OnUpgrade,
{
    type Incoming = RawStatus;
    type Outgoing = RequestLine;

    fn parse(buf: &mut BytesMut) -> ParseResult<RawStatus> {
        if buf.len() == 0 {
            return Ok(None);
        }
        let mut headers_indices = [HeaderIndices {
            name: (0, 0),
            value: (0, 0)
        }; MAX_HEADERS];
        let (len, code, reason, version, headers_len) = {
            let mut headers = [httparse::EMPTY_HEADER; MAX_HEADERS];
            trace!("Response.parse([Header; {}], [u8; {}])", headers.len(), buf.len());
            let mut res = httparse::Response::new(&mut headers);
            let bytes = buf.as_ref();
            match try!(res.parse(bytes)) {
                httparse::Status::Complete(len) => {
                    trace!("Response.parse Complete({})", len);
                    let code = res.code.unwrap();
                    let status = try!(StatusCode::try_from(code).map_err(|_| ::Error::Status));
                    let reason = match status.canonical_reason() {
                        Some(reason) if reason == res.reason.unwrap() => Cow::Borrowed(reason),
                        _ => Cow::Owned(res.reason.unwrap().to_owned())
                    };
                    let version = if res.version.unwrap() == 1 { Http11 } else { Http10 };
                    record_header_indices(bytes, &res.headers, &mut headers_indices);
                    let headers_len = res.headers.len();
                    (len, code, reason, version, headers_len)
                },
                httparse::Status::Partial => return Ok(None),
            }
        };

        let mut headers = Headers::with_capacity(headers_len);
        let slice = buf.split_to(len).freeze();
        headers.extend(HeadersAsBytesIter {
            headers: headers_indices[..headers_len].iter(),
            slice: slice,
        });
        Ok(Some((MessageHead {
            version: version,
            subject: RawStatus(code, reason),
            headers: headers,
        }, len)))
    }

    fn decoder(inc: &MessageHead<Self::Incoming>, method: &mut Option<Method>) -> ::Result<Decode> {
        // According to https://tools.ietf.org/html/rfc7230#section-3.3.3
        // 1. HEAD responses, and Status 1xx, 204, and 304 cannot have a body.
        // 2. Status 2xx to a CONNECT cannot have a body.
        // 3. Transfer-Encoding: chunked has a chunked body.
        // 4. If multiple differing Content-Length headers or invalid, close connection.
        // 5. Content-Length header has a sized body.
        // 6. (irrelevant to Response)
        // 7. Read till EOF.

        match inc.subject.0 {
            101 => {
                return T::on_decode_upgrade().map(Decode::Final);
            },
            100...199 => {
                trace!("ignoring informational response: {}", inc.subject.0);
                return Ok(Decode::Ignore);
            },
            204 |
            304 => return Ok(Decode::Normal(Decoder::length(0))),
            _ => (),
        }
        match *method {
            Some(Method::Head) => {
                return Ok(Decode::Normal(Decoder::length(0)));
            }
            Some(Method::Connect) => match inc.subject.0 {
                200...299 => {
                    return Ok(Decode::Final(Decoder::length(0)));
                },
                _ => {},
            },
            Some(_) => {},
            None => {
                trace!("Client::decoder is missing the Method");
            }
        }


        if let Some(&header::TransferEncoding(ref codings)) = inc.headers.get() {
            if inc.version == Http10 {
                debug!("HTTP/1.0 has Transfer-Encoding header");
                Err(::Error::Header)
            } else if codings.last() == Some(&header::Encoding::Chunked) {
                Ok(Decode::Normal(Decoder::chunked()))
            } else {
                trace!("not chunked. read till eof");
                Ok(Decode::Normal(Decoder::eof()))
            }
        } else if let Some(&header::ContentLength(len)) = inc.headers.get() {
            Ok(Decode::Normal(Decoder::length(len)))
        } else if inc.headers.has::<header::ContentLength>() {
            debug!("illegal Content-Length: {:?}", inc.headers.get_raw("Content-Length"));
            Err(::Error::Header)
        } else {
            trace!("neither Transfer-Encoding nor Content-Length");
            Ok(Decode::Normal(Decoder::eof()))
        }
    }

    fn encode(mut head: MessageHead<Self::Outgoing>, has_body: bool, method: &mut Option<Method>, dst: &mut Vec<u8>) -> ::Result<Encoder> {
        trace!("Client::encode has_body={}, method={:?}", has_body, method);

        *method = Some(head.subject.0.clone());

        let body = Client::set_length(&mut head, has_body);

        let init_cap = 30 + head.headers.len() * AVERAGE_HEADER_SIZE;
        dst.reserve(init_cap);
        let _ = write!(FastWrite(dst), "{} {}\r\n{}\r\n", head.subject, head.version, head.headers);

        Ok(body)
    }

    fn on_error(_err: &::Error) -> Option<MessageHead<Self::Outgoing>> {
        // we can't tell the server about any errors it creates
        None
    }

    fn should_error_on_parse_eof() -> bool {
        true
    }

    fn should_read_first() -> bool {
        false
    }
}

impl Client<()> {
    fn set_length(head: &mut RequestHead, has_body: bool) -> Encoder {
        if has_body {
            let can_chunked = head.version == Http11
                && (head.subject.0 != Method::Head)
                && (head.subject.0 != Method::Get)
                && (head.subject.0 != Method::Connect);
            set_length(&mut head.headers, can_chunked)
        } else {
            head.headers.remove::<ContentLength>();
            head.headers.remove::<TransferEncoding>();
            Encoder::length(0)
        }
    }
}

fn set_length(headers: &mut Headers, can_chunked: bool) -> Encoder {
    let len = headers.get::<header::ContentLength>().map(|n| **n);

    if let Some(len) = len {
        Encoder::length(len)
    } else if can_chunked {
        let encodings = match headers.get_mut::<header::TransferEncoding>() {
            Some(&mut header::TransferEncoding(ref mut encodings)) => {
                if encodings.last() != Some(&header::Encoding::Chunked) {
                    encodings.push(header::Encoding::Chunked);
                }
                false
            },
            None => true
        };

        if encodings {
            headers.set(header::TransferEncoding(vec![header::Encoding::Chunked]));
        }
        Encoder::chunked()
    } else {
        headers.remove::<TransferEncoding>();
        Encoder::eof()
    }
}

pub trait OnUpgrade {
    fn on_encode_upgrade(head: &mut MessageHead<StatusCode>) -> ::Result<()>;
    fn on_decode_upgrade() -> ::Result<Decoder>;
}

pub enum YesUpgrades {}

pub enum NoUpgrades {}

impl OnUpgrade for YesUpgrades {
    fn on_encode_upgrade(_head: &mut MessageHead<StatusCode>) -> ::Result<()> {
        Ok(())
    }

    fn on_decode_upgrade() -> ::Result<Decoder> {
        debug!("101 response received, upgrading");
        // 101 upgrades always have no body
        Ok(Decoder::length(0))
    }
}

impl OnUpgrade for NoUpgrades {
    fn on_encode_upgrade(head: &mut MessageHead<StatusCode>) -> ::Result<()> {
        error!("response with 101 status code not supported");
        *head = MessageHead::default();
        head.subject = ::StatusCode::InternalServerError;
        head.headers.set(ContentLength(0));
        Err(::Error::Status)
    }

    fn on_decode_upgrade() -> ::Result<Decoder> {
        debug!("received 101 upgrade response, not supported");
        return Err(::Error::Upgrade);
    }
}

#[derive(Clone, Copy)]
struct HeaderIndices {
    name: (usize, usize),
    value: (usize, usize),
}

fn record_header_indices(bytes: &[u8], headers: &[httparse::Header], indices: &mut [HeaderIndices]) {
    let bytes_ptr = bytes.as_ptr() as usize;
    for (header, indices) in headers.iter().zip(indices.iter_mut()) {
        let name_start = header.name.as_ptr() as usize - bytes_ptr;
        let name_end = name_start + header.name.len();
        indices.name = (name_start, name_end);
        let value_start = header.value.as_ptr() as usize - bytes_ptr;
        let value_end = value_start + header.value.len();
        indices.value = (value_start, value_end);
    }
}

struct HeadersAsBytesIter<'a> {
    headers: ::std::slice::Iter<'a, HeaderIndices>,
    slice: Bytes,
}

impl<'a> Iterator for HeadersAsBytesIter<'a> {
    type Item = (&'a str, Bytes);
    fn next(&mut self) -> Option<Self::Item> {
        self.headers.next().map(|header| {
            let name = unsafe {
                let bytes = ::std::slice::from_raw_parts(
                    self.slice.as_ref().as_ptr().offset(header.name.0 as isize),
                    header.name.1 - header.name.0
                );
                ::std::str::from_utf8_unchecked(bytes)
            };
            (name, self.slice.slice(header.value.0, header.value.1))
        })
    }
}

struct FastWrite<'a>(&'a mut Vec<u8>);

impl<'a> fmt::Write for FastWrite<'a> {
    #[inline]
    fn write_str(&mut self, s: &str) -> fmt::Result {
        extend(self.0, s.as_bytes());
        Ok(())
    }

    #[inline]
    fn write_fmt(&mut self, args: fmt::Arguments) -> fmt::Result {
        fmt::write(self, args)
    }
}

#[inline]
fn extend(dst: &mut Vec<u8>, data: &[u8]) {
    dst.extend_from_slice(data);
}

#[cfg(test)]
mod tests {
    use bytes::BytesMut;

    use proto::{Decode, MessageHead};
    use super::{Decoder, Server as S, Client as C, NoUpgrades, Http1Transaction};
    use header::{ContentLength, TransferEncoding};

    type Server = S<NoUpgrades>;
    type Client = C<NoUpgrades>;

    impl Decode {
        fn final_(self) -> Decoder {
            match self {
                Decode::Final(d) => d,
                other => panic!("expected Final, found {:?}", other),
            }
        }

        fn normal(self) -> Decoder {
            match self {
                Decode::Normal(d) => d,
                other => panic!("expected Normal, found {:?}", other),
            }
        }

        fn ignore(self) {
            match self {
                Decode::Ignore => {},
                other => panic!("expected Ignore, found {:?}", other),
            }
        }
    }

    #[test]
    fn test_parse_request() {
        extern crate pretty_env_logger;
        let _ = pretty_env_logger::try_init();
        let mut raw = BytesMut::from(b"GET /echo HTTP/1.1\r\nHost: hyper.rs\r\n\r\n".to_vec());
        let expected_len = raw.len();
        let (req, len) = Server::parse(&mut raw).unwrap().unwrap();
        assert_eq!(len, expected_len);
        assert_eq!(req.subject.0, ::Method::Get);
        assert_eq!(req.subject.1, "/echo");
        assert_eq!(req.version, ::HttpVersion::Http11);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers.get_raw("Host").map(|raw| &raw[0]), Some(b"hyper.rs".as_ref()));
    }


    #[test]
    fn test_parse_response() {
        extern crate pretty_env_logger;
        let _ = pretty_env_logger::try_init();
        let mut raw = BytesMut::from(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".to_vec());
        let expected_len = raw.len();
        let (req, len) = Client::parse(&mut raw).unwrap().unwrap();
        assert_eq!(len, expected_len);
        assert_eq!(req.subject.0, 200);
        assert_eq!(req.subject.1, "OK");
        assert_eq!(req.version, ::HttpVersion::Http11);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers.get_raw("Content-Length").map(|raw| &raw[0]), Some(b"0".as_ref()));
    }

    #[test]
    fn test_parse_request_errors() {
        let mut raw = BytesMut::from(b"GET htt:p// HTTP/1.1\r\nHost: hyper.rs\r\n\r\n".to_vec());
        Server::parse(&mut raw).unwrap_err();
    }

    #[test]
    fn test_parse_raw_status() {
        let mut raw = BytesMut::from(b"HTTP/1.1 200 OK\r\n\r\n".to_vec());
        let (res, _) = Client::parse(&mut raw).unwrap().unwrap();
        assert_eq!(res.subject.1, "OK");

        let mut raw = BytesMut::from(b"HTTP/1.1 200 Howdy\r\n\r\n".to_vec());
        let (res, _) = Client::parse(&mut raw).unwrap().unwrap();
        assert_eq!(res.subject.1, "Howdy");
    }


    #[test]
    fn test_decoder_request() {
        use super::Decoder;

        let method = &mut None;
        let mut head = MessageHead::<::proto::RequestLine>::default();

        head.subject.0 = ::Method::Get;
        assert_eq!(Decoder::length(0), Server::decoder(&head, method).unwrap().normal());
        assert_eq!(*method, Some(::Method::Get));

        head.subject.0 = ::Method::Post;
        assert_eq!(Decoder::length(0), Server::decoder(&head, method).unwrap().normal());
        assert_eq!(*method, Some(::Method::Post));

        head.headers.set(TransferEncoding::chunked());
        assert_eq!(Decoder::chunked(), Server::decoder(&head, method).unwrap().normal());
        // transfer-encoding and content-length = chunked
        head.headers.set(ContentLength(10));
        assert_eq!(Decoder::chunked(), Server::decoder(&head, method).unwrap().normal());

        head.headers.remove::<TransferEncoding>();
        assert_eq!(Decoder::length(10), Server::decoder(&head, method).unwrap().normal());

        head.headers.set_raw("Content-Length", vec![b"5".to_vec(), b"5".to_vec()]);
        assert_eq!(Decoder::length(5), Server::decoder(&head, method).unwrap().normal());

        head.headers.set_raw("Content-Length", vec![b"10".to_vec(), b"11".to_vec()]);
        Server::decoder(&head, method).unwrap_err();

        head.headers.remove::<ContentLength>();

        head.headers.set_raw("Transfer-Encoding", "gzip");
        Server::decoder(&head, method).unwrap_err();


        // http/1.0
        head.version = ::HttpVersion::Http10;
        head.headers.clear();

        // 1.0 requests can only have bodies if content-length is set
        assert_eq!(Decoder::length(0), Server::decoder(&head, method).unwrap().normal());

        head.headers.set(TransferEncoding::chunked());
        Server::decoder(&head, method).unwrap_err();
        head.headers.remove::<TransferEncoding>();

        head.headers.set(ContentLength(15));
        assert_eq!(Decoder::length(15), Server::decoder(&head, method).unwrap().normal());
    }

    #[test]
    fn test_decoder_response() {
        use super::Decoder;

        let method = &mut Some(::Method::Get);
        let mut head = MessageHead::<::proto::RawStatus>::default();

        head.subject.0 = 204;
        assert_eq!(Decoder::length(0), Client::decoder(&head, method).unwrap().normal());
        head.subject.0 = 304;
        assert_eq!(Decoder::length(0), Client::decoder(&head, method).unwrap().normal());

        head.subject.0 = 200;
        assert_eq!(Decoder::eof(), Client::decoder(&head, method).unwrap().normal());

        *method = Some(::Method::Head);
        assert_eq!(Decoder::length(0), Client::decoder(&head, method).unwrap().normal());

        *method = Some(::Method::Connect);
        assert_eq!(Decoder::length(0), Client::decoder(&head, method).unwrap().final_());


        // CONNECT receiving non 200 can have a body
        head.subject.0 = 404;
        head.headers.set(ContentLength(10));
        assert_eq!(Decoder::length(10), Client::decoder(&head, method).unwrap().normal());
        head.headers.remove::<ContentLength>();


        *method = Some(::Method::Get);
        head.headers.set(TransferEncoding::chunked());
        assert_eq!(Decoder::chunked(), Client::decoder(&head, method).unwrap().normal());

        // transfer-encoding and content-length = chunked
        head.headers.set(ContentLength(10));
        assert_eq!(Decoder::chunked(), Client::decoder(&head, method).unwrap().normal());

        head.headers.remove::<TransferEncoding>();
        assert_eq!(Decoder::length(10), Client::decoder(&head, method).unwrap().normal());

        head.headers.set_raw("Content-Length", vec![b"5".to_vec(), b"5".to_vec()]);
        assert_eq!(Decoder::length(5), Client::decoder(&head, method).unwrap().normal());

        head.headers.set_raw("Content-Length", vec![b"10".to_vec(), b"11".to_vec()]);
        Client::decoder(&head, method).unwrap_err();
        head.headers.clear();

        // 1xx status codes
        head.subject.0 = 100;
        Client::decoder(&head, method).unwrap().ignore();

        head.subject.0 = 103;
        Client::decoder(&head, method).unwrap().ignore();

        // 101 upgrade not supported yet
        head.subject.0 = 101;
        Client::decoder(&head, method).unwrap_err();
        head.subject.0 = 200;

        // http/1.0
        head.version = ::HttpVersion::Http10;

        assert_eq!(Decoder::eof(), Client::decoder(&head, method).unwrap().normal());

        head.headers.set(TransferEncoding::chunked());
        Client::decoder(&head, method).unwrap_err();
    }

    #[cfg(feature = "nightly")]
    use test::Bencher;

    #[cfg(feature = "nightly")]
    #[bench]
    fn bench_parse_incoming(b: &mut Bencher) {
        let mut raw = BytesMut::from(
            b"GET /super_long_uri/and_whatever?what_should_we_talk_about/\
            I_wonder/Hard_to_write_in_an_uri_after_all/you_have_to_make\
            _up_the_punctuation_yourself/how_fun_is_that?test=foo&test1=\
            foo1&test2=foo2&test3=foo3&test4=foo4 HTTP/1.1\r\nHost: \
            hyper.rs\r\nAccept: a lot of things\r\nAccept-Charset: \
            utf8\r\nAccept-Encoding: *\r\nAccess-Control-Allow-\
            Credentials: None\r\nAccess-Control-Allow-Origin: None\r\n\
            Access-Control-Allow-Methods: None\r\nAccess-Control-Allow-\
            Headers: None\r\nContent-Encoding: utf8\r\nContent-Security-\
            Policy: None\r\nContent-Type: text/html\r\nOrigin: hyper\
            \r\nSec-Websocket-Extensions: It looks super important!\r\n\
            Sec-Websocket-Origin: hyper\r\nSec-Websocket-Version: 4.3\r\
            \nStrict-Transport-Security: None\r\nUser-Agent: hyper\r\n\
            X-Content-Duration: None\r\nX-Content-Security-Policy: None\
            \r\nX-DNSPrefetch-Control: None\r\nX-Frame-Options: \
            Something important obviously\r\nX-Requested-With: Nothing\
            \r\n\r\n".to_vec()
        );
        let len = raw.len();

        b.bytes = len as u64;
        b.iter(|| {
            Server::parse(&mut raw).unwrap();
            restart(&mut raw, len);
        });


        fn restart(b: &mut BytesMut, len: usize) {
            b.reserve(1);
            unsafe {
                b.set_len(len);
            }
        }
    }

    #[cfg(feature = "nightly")]
    #[bench]
    fn bench_server_transaction_encode(b: &mut Bencher) {
        use header::{Headers, ContentLength, ContentType};
        use ::{StatusCode, HttpVersion};

        let len = 108;
        b.bytes = len as u64;

        let mut head = MessageHead {
            subject: StatusCode::Ok,
            headers: Headers::new(),
            version: HttpVersion::Http11,
        };
        head.headers.set(ContentLength(10));
        head.headers.set(ContentType::json());

        b.iter(|| {
            let mut vec = Vec::new();
            Server::encode(head.clone(), true, &mut None, &mut vec).unwrap();
            assert_eq!(vec.len(), len);
            ::test::black_box(vec);
        })
    }
}
