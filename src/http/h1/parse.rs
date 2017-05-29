use std::borrow::Cow;
use std::fmt::{self, Write};

use httparse;
use bytes::{BytesMut, Bytes};

use header::{self, Headers, ContentLength, TransferEncoding};
use http::{ByteStr, MessageHead, RawStatus, Http1Transaction, ParseResult, ServerTransaction, ClientTransaction, RequestLine};
use http::h1::{Encoder, Decoder, date};
use method::Method;
use status::StatusCode;
use version::HttpVersion::{Http10, Http11};

const MAX_HEADERS: usize = 100;
const AVERAGE_HEADER_SIZE: usize = 30; // totally scientific

impl Http1Transaction for ServerTransaction {
    type Incoming = RequestLine;
    type Outgoing = StatusCode;

    fn parse(buf: &mut BytesMut) -> ParseResult<RequestLine> {
        if buf.len() == 0 {
            return Ok(None);
        }
        trace!("parse({:?})", buf);
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
                    trace!("httparse Complete({})", len);
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
        let path = unsafe { ByteStr::from_utf8_unchecked(path) };
        let subject = RequestLine(
            method,
            try!(::uri::from_byte_str(path)),
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

    fn decoder(head: &MessageHead<Self::Incoming>) -> ::Result<Decoder> {
        use ::header;
        if let Some(&header::ContentLength(len)) = head.headers.get() {
            Ok(Decoder::length(len))
        } else if head.headers.has::<header::TransferEncoding>() {
            //TODO: check for Transfer-Encoding: chunked
            Ok(Decoder::chunked())
        } else {
            Ok(Decoder::length(0))
        }
    }


    fn encode(mut head: MessageHead<Self::Outgoing>, dst: &mut Vec<u8>) -> Encoder {
        use ::header;
        trace!("writing head: {:?}", head);

        let len = head.headers.get::<header::ContentLength>().map(|n| **n);

        let body = if let Some(len) = len {
            Encoder::length(len)
        } else {
            let encodings = match head.headers.get_mut::<header::TransferEncoding>() {
                Some(&mut header::TransferEncoding(ref mut encodings)) => {
                    if encodings.last() != Some(&header::Encoding::Chunked) {
                        encodings.push(header::Encoding::Chunked);
                    }
                    false
                },
                None => true
            };

            if encodings {
                head.headers.set(header::TransferEncoding(vec![header::Encoding::Chunked]));
            }
            Encoder::chunked()
        };


        let init_cap = 30 + head.headers.len() * AVERAGE_HEADER_SIZE;
        dst.reserve(init_cap);
        debug!("writing headers = {:?}", head.headers);
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
        body
    }

    fn should_set_length(_head: &MessageHead<Self::Outgoing>) -> bool {
        //TODO: pass method, check if method == HEAD
        true
    }
}

impl Http1Transaction for ClientTransaction {
    type Incoming = RawStatus;
    type Outgoing = RequestLine;

    fn parse(buf: &mut BytesMut) -> ParseResult<RawStatus> {
        if buf.len() == 0 {
            return Ok(None);
        }
        trace!("parse({:?})", buf);
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
                    trace!("Response.try_parse Complete({})", len);
                    let code = res.code.unwrap();
                    let reason = match StatusCode::from_u16(code).canonical_reason() {
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

    fn decoder(inc: &MessageHead<Self::Incoming>) -> ::Result<Decoder> {
        use ::header;
        // According to https://tools.ietf.org/html/rfc7230#section-3.3.3
        // 1. HEAD responses, and Status 1xx, 204, and 304 cannot have a body.
        // 2. Status 2xx to a CONNECT cannot have a body.
        //
        // First two steps taken care of before this method.
        //
        // 3. Transfer-Encoding: chunked has a chunked body.
        // 4. If multiple differing Content-Length headers or invalid, close connection.
        // 5. Content-Length header has a sized body.
        // 6. Not Client.
        // 7. Read till EOF.
        if let Some(&header::TransferEncoding(ref codings)) = inc.headers.get() {
            if codings.last() == Some(&header::Encoding::Chunked) {
                Ok(Decoder::chunked())
            } else {
                trace!("not chunked. read till eof");
                Ok(Decoder::eof())
            }
        } else if let Some(&header::ContentLength(len)) = inc.headers.get() {
            Ok(Decoder::length(len))
        } else if inc.headers.has::<header::ContentLength>() {
            trace!("illegal Content-Length: {:?}", inc.headers.get_raw("Content-Length"));
            Err(::Error::Header)
        } else {
            trace!("neither Transfer-Encoding nor Content-Length");
            Ok(Decoder::eof())
        }
    }

    fn encode(mut head: MessageHead<Self::Outgoing>, dst: &mut Vec<u8>) -> Encoder {
        trace!("writing head: {:?}", head);


        let mut body = Encoder::length(0);
        let expects_no_body = match head.subject.0 {
            Method::Head | Method::Get | Method::Connect => true,
            _ => false
        };
        let mut chunked = false;

        if let Some(con_len) = head.headers.get::<ContentLength>() {
            body = Encoder::length(**con_len);
        } else {
            chunked = !expects_no_body;
        }

        if chunked {
            body = Encoder::chunked();
            let encodings = match head.headers.get_mut::<TransferEncoding>() {
                Some(encodings) => {
                    if !encodings.contains(&header::Encoding::Chunked) {
                        encodings.push(header::Encoding::Chunked);
                    }
                    true
                },
                None => false
            };

            if !encodings {
                head.headers.set(TransferEncoding(vec![header::Encoding::Chunked]));
            }
        }

        let init_cap = 30 + head.headers.len() * AVERAGE_HEADER_SIZE;
        dst.reserve(init_cap);
        debug!("writing {:#?}", head.headers);
        let _ = write!(FastWrite(dst), "{} {}\r\n{}\r\n", head.subject, head.version, head.headers);

        body
    }


    fn should_set_length(head: &MessageHead<Self::Outgoing>) -> bool {
        match &head.subject.0 {
            &Method::Get | &Method::Head => false,
            _ => true
        }
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
    fn write_str(&mut self, s: &str) -> fmt::Result {
        extend(self.0, s.as_bytes());
        Ok(())
    }

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
    use http::{ServerTransaction, ClientTransaction, Http1Transaction};
    use bytes::BytesMut;

    #[test]
    fn test_parse_request() {
        extern crate pretty_env_logger;
        let _ = pretty_env_logger::init();
        let mut raw = BytesMut::from(b"GET /echo HTTP/1.1\r\nHost: hyper.rs\r\n\r\n".to_vec());
        let expected_len = raw.len();
        let (req, len) = ServerTransaction::parse(&mut raw).unwrap().unwrap();
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
        let _ = pretty_env_logger::init();
        let mut raw = BytesMut::from(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".to_vec());
        let expected_len = raw.len();
        let (req, len) = ClientTransaction::parse(&mut raw).unwrap().unwrap();
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
        ServerTransaction::parse(&mut raw).unwrap_err();
    }
    #[test]
    fn test_parse_raw_status() {
        let mut raw = BytesMut::from(b"HTTP/1.1 200 OK\r\n\r\n".to_vec());
        let (res, _) = ClientTransaction::parse(&mut raw).unwrap().unwrap();
        assert_eq!(res.subject.1, "OK");

        let mut raw = BytesMut::from(b"HTTP/1.1 200 Howdy\r\n\r\n".to_vec());
        let (res, _) = ClientTransaction::parse(&mut raw).unwrap().unwrap();
        assert_eq!(res.subject.1, "Howdy");
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
            ServerTransaction::parse(&mut raw).unwrap();
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
        use ::http::MessageHead;
        use ::header::{Headers, ContentLength};
        use ::{StatusCode, HttpVersion};
        b.bytes = 75;

        let mut head = MessageHead {
            subject: StatusCode::Ok,
            headers: Headers::new(),
            version: HttpVersion::Http11,
        };
        head.headers.set(ContentLength(0));

        b.iter(|| {
            let mut vec = Vec::new();
            ServerTransaction::encode(head.clone(), &mut vec);
            assert_eq!(vec.len(), 75);
            ::test::black_box(vec);
        })
    }
}
