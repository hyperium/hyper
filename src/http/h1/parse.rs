use std::borrow::Cow;
use std::fmt::{self, Write};

use httparse;

use header::{self, Headers, ContentLength, TransferEncoding};
use http::{MessageHead, RawStatus, Http1Message, ParseResult, ServerMessage, ClientMessage, RequestLine};
use http::h1::{Encoder, Decoder};
use method::Method;
use status::StatusCode;
use version::HttpVersion::{Http10, Http11};

const MAX_HEADERS: usize = 100;
const AVERAGE_HEADER_SIZE: usize = 30; // totally scientific

pub fn parse<T: Http1Message<Incoming=I>, I>(buf: &[u8]) -> ParseResult<I> {
    if buf.len() == 0 {
        return Ok(None);
    }
    trace!("parse({:?})", buf);
    <T as Http1Message>::parse(buf)
}



impl Http1Message for ServerMessage {
    type Incoming = RequestLine;
    type Outgoing = StatusCode;

    fn parse(buf: &[u8]) -> ParseResult<RequestLine> {
        let mut headers = [httparse::EMPTY_HEADER; MAX_HEADERS];
        trace!("Request.parse([Header; {}], [u8; {}])", headers.len(), buf.len());
        let mut req = httparse::Request::new(&mut headers);
        Ok(match try!(req.parse(buf)) {
            httparse::Status::Complete(len) => {
                trace!("Request.parse Complete({})", len);
                Some((MessageHead {
                    version: if req.version.unwrap() == 1 { Http11 } else { Http10 },
                    subject: RequestLine(
                        try!(req.method.unwrap().parse()),
                        try!(req.path.unwrap().parse())
                    ),
                    headers: try!(Headers::from_raw(req.headers))
                }, len))
            },
            httparse::Status::Partial => None
        })
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

        if !head.headers.has::<header::Date>() {
            head.headers.set(header::Date(header::HttpDate(::time::now_utc())));
        }

        let mut is_chunked = true;
        let mut body = Encoder::chunked();
        if let Some(cl) = head.headers.get::<header::ContentLength>() {
            body = Encoder::length(**cl);
            is_chunked = false
        }

        if is_chunked {
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
        }


        let init_cap = 30 + head.headers.len() * AVERAGE_HEADER_SIZE;
        dst.reserve(init_cap);
        debug!("writing {:#?}", head.headers);
        if head.version == ::HttpVersion::Http11 && head.subject == ::StatusCode::Ok {
            extend(dst, b"HTTP/1.1 200 OK\r\n");
            let _ = write!(FastWrite(dst), "{}\r\n", head.headers);
        } else {
            let _ = write!(FastWrite(dst), "{} {}\r\n{}\r\n", head.version, head.subject, head.headers);
        }
        body
    }
}

impl Http1Message for ClientMessage {
    type Incoming = RawStatus;
    type Outgoing = RequestLine;

    fn parse(buf: &[u8]) -> ParseResult<RawStatus> {
        let mut headers = [httparse::EMPTY_HEADER; MAX_HEADERS];
        trace!("Response.parse([Header; {}], [u8; {}])", headers.len(), buf.len());
        let mut res = httparse::Response::new(&mut headers);
        Ok(match try!(res.parse(buf)) {
            httparse::Status::Complete(len) => {
                trace!("Response.try_parse Complete({})", len);
                let code = res.code.unwrap();
                let reason = match StatusCode::from_u16(code).canonical_reason() {
                    Some(reason) if reason == res.reason.unwrap() => Cow::Borrowed(reason),
                    _ => Cow::Owned(res.reason.unwrap().to_owned())
                };
                Some((MessageHead {
                    version: if res.version.unwrap() == 1 { Http11 } else { Http10 },
                    subject: RawStatus(code, reason),
                    headers: try!(Headers::from_raw(res.headers))
                }, len))
            },
            httparse::Status::Partial => None
        })
    }

    fn decoder(inc: &MessageHead<Self::Incoming>) -> ::Result<Decoder> {
        use ::header;
        // According to https://tools.ietf.org/html/rfc7230#section-3.3.3
        // 1. HEAD reponses, and Status 1xx, 204, and 304 cannot have a body.
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
                trace!("not chuncked. read till eof");
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

fn extend(dst: &mut Vec<u8>, data: &[u8]) {
    use std::ptr;
    dst.reserve(data.len());
    let prev = dst.len();
    unsafe {
        ptr::copy_nonoverlapping(data.as_ptr(),
                                 dst.as_mut_ptr().offset(prev as isize),
                                 data.len());
        dst.set_len(prev + data.len());
    }
}

#[cfg(test)]
mod tests {
    use http;
    use super::{parse};

    #[test]
    fn test_parse_request() {
        let raw = b"GET /echo HTTP/1.1\r\nHost: hyper.rs\r\n\r\n";
        parse::<http::ServerMessage, _>(raw).unwrap();
    }

    #[test]
    fn test_parse_raw_status() {
        let raw = b"HTTP/1.1 200 OK\r\n\r\n";
        let (res, _) = parse::<http::ClientMessage, _>(raw).unwrap().unwrap();
        assert_eq!(res.subject.1, "OK");

        let raw = b"HTTP/1.1 200 Howdy\r\n\r\n";
        let (res, _) = parse::<http::ClientMessage, _>(raw).unwrap().unwrap();
        assert_eq!(res.subject.1, "Howdy");
    }

    #[cfg(feature = "nightly")]
    use test::Bencher;

    #[cfg(feature = "nightly")]
    #[bench]
    fn bench_parse_incoming(b: &mut Bencher) {
        let raw = b"GET /echo HTTP/1.1\r\nHost: hyper.rs\r\n\r\n";
        b.iter(|| {
            parse::<http::ServerMessage, _>(raw).unwrap()
        });
    }

}
