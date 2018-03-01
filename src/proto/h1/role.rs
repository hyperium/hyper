use std::fmt::{self, Write};

use bytes::{BytesMut, Bytes};
use http::header::{CONTENT_LENGTH, DATE, HeaderName, HeaderValue, TRANSFER_ENCODING};
use http::{HeaderMap, Method, StatusCode, Uri, Version};
use httparse;

use headers;
use proto::{Decode, MessageHead, Http1Transaction, ParseResult, RequestLine, RequestHead};
use proto::h1::{Encoder, Decoder, date};

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
                    let method = Method::from_bytes(req.method.unwrap().as_bytes())?;
                    let path = req.path.unwrap();
                    let bytes_ptr = buf.as_ref().as_ptr() as usize;
                    let path_start = path.as_ptr() as usize - bytes_ptr;
                    let path_end = path_start + path.len();
                    let path = (path_start, path_end);
                    let version = if req.version.unwrap() == 1 {
                        Version::HTTP_11
                    } else {
                        Version::HTTP_10
                    };

                    record_header_indices(buf.as_ref(), &req.headers, &mut headers_indices);
                    let headers_len = req.headers.len();
                    (len, method, path, version, headers_len)
                }
                httparse::Status::Partial => return Ok(None),
            }
        };

        let mut headers = HeaderMap::with_capacity(headers_len);
        let slice = buf.split_to(len).freeze();
        let path = slice.slice(path.0, path.1);
        // path was found to be utf8 by httparse
        let path = Uri::from_shared(path)?;
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
        *method = Some(head.subject.0.clone());

        // According to https://tools.ietf.org/html/rfc7230#section-3.3.3
        // 1. (irrelevant to Request)
        // 2. (irrelevant to Request)
        // 3. Transfer-Encoding: chunked has a chunked body.
        // 4. If multiple differing Content-Length headers or invalid, close connection.
        // 5. Content-Length header has a sized body.
        // 6. Length 0.
        // 7. (irrelevant to Request)

        if head.headers.contains_key(TRANSFER_ENCODING) {
            // https://tools.ietf.org/html/rfc7230#section-3.3.3
            // If Transfer-Encoding header is present, and 'chunked' is
            // not the final encoding, and this is a Request, then it is
            // mal-formed. A server should respond with 400 Bad Request.
            if head.version == Version::HTTP_10 {
                debug!("HTTP/1.0 cannot have Transfer-Encoding header");
                Err(::Error::Header)
            } else if headers::transfer_encoding_is_chunked(&head.headers) {
                Ok(Decode::Normal(Decoder::chunked()))
            } else {
                debug!("request with transfer-encoding header, but not chunked, bad request");
                Err(::Error::Header)
            }
        } else if let Some(len) = headers::content_length_parse(&head.headers) {
            Ok(Decode::Normal(Decoder::length(len)))
        } else if head.headers.contains_key(CONTENT_LENGTH) {
            debug!("illegal Content-Length header");
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
        let ret = if StatusCode::SWITCHING_PROTOCOLS == head.subject {
            T::on_encode_upgrade(&mut head)
                .map(|_| {
                    let mut enc = Server::set_length(&mut head, has_body, method.as_ref());
                    enc.set_last();
                    enc
                })
        } else if head.subject.is_informational() {
            error!("response with 1xx status code not supported");
            head = MessageHead::default();
            head.subject = StatusCode::INTERNAL_SERVER_ERROR;
            headers::content_length_zero(&mut head.headers);
            Err(::Error::Status)
        } else {
            Ok(Server::set_length(&mut head, has_body, method.as_ref()))
        };


        let init_cap = 30 + head.headers.len() * AVERAGE_HEADER_SIZE;
        dst.reserve(init_cap);
        if head.version == Version::HTTP_11 && head.subject == StatusCode::OK {
            extend(dst, b"HTTP/1.1 200 OK\r\n");
        } else {
            match head.version {
                Version::HTTP_10 => extend(dst, b"HTTP/1.0 "),
                Version::HTTP_11 => extend(dst, b"HTTP/1.1 "),
                _ => unreachable!(),
            }

            extend(dst, head.subject.as_str().as_bytes());
            extend(dst, b" ");
            extend(dst, head.subject.canonical_reason().unwrap_or("<none>").as_bytes());
            extend(dst, b"\r\n");
        }
        write_headers(&head.headers, dst);
        // using http::h1::date is quite a lot faster than generating a unique Date header each time
        // like req/s goes up about 10%
        if !head.headers.contains_key(DATE) {
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
            &::Error::Header /*|
            &::Error::Uri(_)*/ => {
                StatusCode::BAD_REQUEST
            },
            &::Error::TooLarge => {
                StatusCode::REQUEST_HEADER_FIELDS_TOO_LARGE
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
        const HEAD: Option<&'static Method> = Some(&Method::HEAD);
        const CONNECT: Option<&'static Method> = Some(&Method::CONNECT);

        let can_have_body = {
            if method == HEAD {
                false
            } else if method == CONNECT && head.subject.is_success() {
                false
            } else {
                match head.subject {
                    // TODO: support for 1xx codes needs improvement everywhere
                    // would be 100...199 => false
                    StatusCode::SWITCHING_PROTOCOLS |
                    StatusCode::NO_CONTENT |
                    StatusCode::NOT_MODIFIED => false,
                    _ => true,
                }
            }
        };

        if has_body && can_have_body {
            set_length(&mut head.headers, head.version == Version::HTTP_11)
        } else {
            head.headers.remove(TRANSFER_ENCODING);
            if can_have_body {
                headers::content_length_zero(&mut head.headers);
            }
            Encoder::length(0)
        }
    }
}

impl<T> Http1Transaction for Client<T>
where
    T: OnUpgrade,
{
    type Incoming = StatusCode;
    type Outgoing = RequestLine;

    fn parse(buf: &mut BytesMut) -> ParseResult<StatusCode> {
        if buf.len() == 0 {
            return Ok(None);
        }
        let mut headers_indices = [HeaderIndices {
            name: (0, 0),
            value: (0, 0)
        }; MAX_HEADERS];
        let (len, status, version, headers_len) = {
            let mut headers = [httparse::EMPTY_HEADER; MAX_HEADERS];
            trace!("Response.parse([Header; {}], [u8; {}])", headers.len(), buf.len());
            let mut res = httparse::Response::new(&mut headers);
            let bytes = buf.as_ref();
            match try!(res.parse(bytes)) {
                httparse::Status::Complete(len) => {
                    trace!("Response.parse Complete({})", len);
                    let status = try!(StatusCode::from_u16(res.code.unwrap()).map_err(|_| ::Error::Status));
                    let version = if res.version.unwrap() == 1 {
                        Version::HTTP_11
                    } else {
                        Version::HTTP_10
                    };
                    record_header_indices(bytes, &res.headers, &mut headers_indices);
                    let headers_len = res.headers.len();
                    (len, status, version, headers_len)
                },
                httparse::Status::Partial => return Ok(None),
            }
        };

        let mut headers = HeaderMap::with_capacity(headers_len);
        let slice = buf.split_to(len).freeze();
        headers.extend(HeadersAsBytesIter {
            headers: headers_indices[..headers_len].iter(),
            slice: slice,
        });
        Ok(Some((MessageHead {
            version: version,
            subject: status,
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

        match inc.subject.as_u16() {
            101 => {
                return T::on_decode_upgrade().map(Decode::Final);
            },
            100...199 => {
                trace!("ignoring informational response: {}", inc.subject.as_u16());
                return Ok(Decode::Ignore);
            },
            204 |
            304 => return Ok(Decode::Normal(Decoder::length(0))),
            _ => (),
        }
        match *method {
            Some(Method::HEAD) => {
                return Ok(Decode::Normal(Decoder::length(0)));
            }
            Some(Method::CONNECT) => match inc.subject.as_u16() {
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

        if inc.headers.contains_key(TRANSFER_ENCODING) {
            // https://tools.ietf.org/html/rfc7230#section-3.3.3
            // If Transfer-Encoding header is present, and 'chunked' is
            // not the final encoding, and this is a Request, then it is
            // mal-formed. A server should respond with 400 Bad Request.
            if inc.version == Version::HTTP_10 {
                debug!("HTTP/1.0 cannot have Transfer-Encoding header");
                Err(::Error::Header)
            } else if headers::transfer_encoding_is_chunked(&inc.headers) {
                Ok(Decode::Normal(Decoder::chunked()))
            } else {
                trace!("not chunked, read till eof");
                Ok(Decode::Normal(Decoder::eof()))
            }
        } else if let Some(len) = headers::content_length_parse(&inc.headers) {
            Ok(Decode::Normal(Decoder::length(len)))
        } else if inc.headers.contains_key(CONTENT_LENGTH) {
            debug!("illegal Content-Length header");
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


        extend(dst, head.subject.0.as_str().as_bytes());
        extend(dst, b" ");
        //TODO: add API to http::Uri to encode without std::fmt
        let _ = write!(FastWrite(dst), "{} ", head.subject.1);

        match head.version {
            Version::HTTP_10 => extend(dst, b"HTTP/1.0"),
            Version::HTTP_11 => extend(dst, b"HTTP/1.1"),
            _ => unreachable!(),
        }
        extend(dst, b"\r\n");

        write_headers(&head.headers, dst);
        extend(dst, b"\r\n");

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
            let can_chunked = head.version == Version::HTTP_11
                && (head.subject.0 != Method::HEAD)
                && (head.subject.0 != Method::GET)
                && (head.subject.0 != Method::CONNECT);
            set_length(&mut head.headers, can_chunked)
        } else {
            head.headers.remove(CONTENT_LENGTH);
            head.headers.remove(TRANSFER_ENCODING);
            Encoder::length(0)
        }
    }
}

fn set_length(headers: &mut HeaderMap, can_chunked: bool) -> Encoder {
    let len = headers::content_length_parse(&headers);

    if let Some(len) = len {
        Encoder::length(len)
    } else if can_chunked {
        //TODO: maybe not overwrite existing transfer-encoding
        headers.insert(TRANSFER_ENCODING, HeaderValue::from_static("chunked"));
        Encoder::chunked()
    } else {
        headers.remove(TRANSFER_ENCODING);
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
        head.subject = ::StatusCode::INTERNAL_SERVER_ERROR;
        headers::content_length_zero(&mut head.headers);
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
    type Item = (HeaderName, HeaderValue);
    fn next(&mut self) -> Option<Self::Item> {
        self.headers.next().map(|header| {
            let name = unsafe {
                let bytes = ::std::slice::from_raw_parts(
                    self.slice.as_ref().as_ptr().offset(header.name.0 as isize),
                    header.name.1 - header.name.0
                );
                ::std::str::from_utf8_unchecked(bytes)
            };
            let name = HeaderName::from_bytes(name.as_bytes())
                .expect("header name already validated");
            let value = unsafe {
                HeaderValue::from_shared_unchecked(
                    self.slice.slice(header.value.0, header.value.1)
                )
            };
            (name, value)
        })
    }
}

fn write_headers(headers: &HeaderMap, dst: &mut Vec<u8>) {
    for (name, value) in headers {
        extend(dst, name.as_str().as_bytes());
        extend(dst, b": ");
        extend(dst, value.as_bytes());
        extend(dst, b"\r\n");
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
        assert_eq!(req.subject.0, ::Method::GET);
        assert_eq!(req.subject.1, "/echo");
        assert_eq!(req.version, ::Version::HTTP_11);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers["Host"], "hyper.rs");
    }


    #[test]
    fn test_parse_response() {
        extern crate pretty_env_logger;
        let _ = pretty_env_logger::try_init();
        let mut raw = BytesMut::from(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".to_vec());
        let expected_len = raw.len();
        let (req, len) = Client::parse(&mut raw).unwrap().unwrap();
        assert_eq!(len, expected_len);
        assert_eq!(req.subject, ::StatusCode::OK);
        assert_eq!(req.version, ::Version::HTTP_11);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers["Content-Length"], "0");
    }

    #[test]
    fn test_parse_request_errors() {
        let mut raw = BytesMut::from(b"GET htt:p// HTTP/1.1\r\nHost: hyper.rs\r\n\r\n".to_vec());
        Server::parse(&mut raw).unwrap_err();
    }

    #[test]
    fn test_decoder_request() {
        use super::Decoder;

        let method = &mut None;
        let mut head = MessageHead::<::proto::RequestLine>::default();

        head.subject.0 = ::Method::GET;
        assert_eq!(Decoder::length(0), Server::decoder(&head, method).unwrap().normal());
        assert_eq!(*method, Some(::Method::GET));

        head.subject.0 = ::Method::POST;
        assert_eq!(Decoder::length(0), Server::decoder(&head, method).unwrap().normal());
        assert_eq!(*method, Some(::Method::POST));

        head.headers.insert("transfer-encoding", ::http::header::HeaderValue::from_static("chunked"));
        assert_eq!(Decoder::chunked(), Server::decoder(&head, method).unwrap().normal());
        // transfer-encoding and content-length = chunked
        head.headers.insert("content-length", ::http::header::HeaderValue::from_static("10"));
        assert_eq!(Decoder::chunked(), Server::decoder(&head, method).unwrap().normal());

        head.headers.remove("transfer-encoding");
        assert_eq!(Decoder::length(10), Server::decoder(&head, method).unwrap().normal());

        head.headers.insert("content-length", ::http::header::HeaderValue::from_static("5"));
        head.headers.append("content-length", ::http::header::HeaderValue::from_static("5"));
        assert_eq!(Decoder::length(5), Server::decoder(&head, method).unwrap().normal());

        head.headers.insert("content-length", ::http::header::HeaderValue::from_static("5"));
        head.headers.append("content-length", ::http::header::HeaderValue::from_static("6"));
        Server::decoder(&head, method).unwrap_err();

        head.headers.remove("content-length");

        head.headers.insert("transfer-encoding", ::http::header::HeaderValue::from_static("gzip"));
        Server::decoder(&head, method).unwrap_err();


        // http/1.0
        head.version = ::Version::HTTP_10;
        head.headers.clear();

        // 1.0 requests can only have bodies if content-length is set
        assert_eq!(Decoder::length(0), Server::decoder(&head, method).unwrap().normal());

        head.headers.insert("transfer-encoding", ::http::header::HeaderValue::from_static("chunked"));
        Server::decoder(&head, method).unwrap_err();
        head.headers.remove("transfer-encoding");

        head.headers.insert("content-length", ::http::header::HeaderValue::from_static("15"));
        assert_eq!(Decoder::length(15), Server::decoder(&head, method).unwrap().normal());
    }

    #[test]
    fn test_decoder_response() {
        use super::Decoder;

        let method = &mut Some(::Method::GET);
        let mut head = MessageHead::<::StatusCode>::default();

        head.subject = ::StatusCode::from_u16(204).unwrap();
        assert_eq!(Decoder::length(0), Client::decoder(&head, method).unwrap().normal());
        head.subject = ::StatusCode::from_u16(304).unwrap();
        assert_eq!(Decoder::length(0), Client::decoder(&head, method).unwrap().normal());

        head.subject = ::StatusCode::OK;
        assert_eq!(Decoder::eof(), Client::decoder(&head, method).unwrap().normal());

        *method = Some(::Method::HEAD);
        assert_eq!(Decoder::length(0), Client::decoder(&head, method).unwrap().normal());

        *method = Some(::Method::CONNECT);
        assert_eq!(Decoder::length(0), Client::decoder(&head, method).unwrap().final_());


        // CONNECT receiving non 200 can have a body
        head.subject = ::StatusCode::NOT_FOUND;
        head.headers.insert("content-length", ::http::header::HeaderValue::from_static("10"));
        assert_eq!(Decoder::length(10), Client::decoder(&head, method).unwrap().normal());
        head.headers.remove("content-length");


        *method = Some(::Method::GET);
        head.headers.insert("transfer-encoding", ::http::header::HeaderValue::from_static("chunked"));
        assert_eq!(Decoder::chunked(), Client::decoder(&head, method).unwrap().normal());

        // transfer-encoding and content-length = chunked
        head.headers.insert("content-length", ::http::header::HeaderValue::from_static("10"));
        assert_eq!(Decoder::chunked(), Client::decoder(&head, method).unwrap().normal());

        head.headers.remove("transfer-encoding");
        assert_eq!(Decoder::length(10), Client::decoder(&head, method).unwrap().normal());

        head.headers.insert("content-length", ::http::header::HeaderValue::from_static("5"));
        head.headers.append("content-length", ::http::header::HeaderValue::from_static("5"));
        assert_eq!(Decoder::length(5), Client::decoder(&head, method).unwrap().normal());

        head.headers.insert("content-length", ::http::header::HeaderValue::from_static("5"));
        head.headers.append("content-length", ::http::header::HeaderValue::from_static("6"));
        Client::decoder(&head, method).unwrap_err();
        head.headers.clear();

        // 1xx status codes
        head.subject = ::StatusCode::CONTINUE;
        Client::decoder(&head, method).unwrap().ignore();

        head.subject = ::StatusCode::from_u16(103).unwrap();
        Client::decoder(&head, method).unwrap().ignore();

        // 101 upgrade not supported yet
        head.subject = ::StatusCode::SWITCHING_PROTOCOLS;
        Client::decoder(&head, method).unwrap_err();
        head.subject = ::StatusCode::OK;

        // http/1.0
        head.version = ::Version::HTTP_10;

        assert_eq!(Decoder::eof(), Client::decoder(&head, method).unwrap().normal());

        head.headers.insert("transfer-encoding", ::http::header::HeaderValue::from_static("chunked"));
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
        use http::header::HeaderValue;
        use proto::BodyLength;

        let len = 108;
        b.bytes = len as u64;

        let mut head = MessageHead::default();
        head.headers.insert("content-length", HeaderValue::from_static("10"));
        head.headers.insert("content-type", HeaderValue::from_static("application/json"));

        b.iter(|| {
            let mut vec = Vec::new();
            Server::encode(head.clone(), Some(BodyLength::Known(10)), &mut None, &mut vec).unwrap();
            assert_eq!(vec.len(), len);
            ::test::black_box(vec);
        })
    }
}
