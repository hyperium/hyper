use std::fmt::{self, Write};
use std::mem;

use bytes::{BytesMut, Bytes};
use http::header::{self, Entry, HeaderName, HeaderValue};
use http::{HeaderMap, Method, StatusCode, Version};
use httparse;

use error::Parse;
use headers;
use proto::{BodyLength, MessageHead, RequestLine, RequestHead};
use proto::h1::{Decode, Decoder, Encode, Encoder, Http1Transaction, ParseResult, ParseContext, ParsedMessage, date};

const MAX_HEADERS: usize = 100;
const AVERAGE_HEADER_SIZE: usize = 30; // totally scientific

// There are 2 main roles, Client and Server.
//
// There is 1 modifier, OnUpgrade, which can wrap Client and Server,
// to signal that HTTP upgrades are not supported.

pub(crate) struct Client<T>(T);

pub(crate) struct Server<T>(T);

impl<T> Http1Transaction for Server<T>
where
    T: OnUpgrade,
{
    type Incoming = RequestLine;
    type Outgoing = StatusCode;

    fn parse(buf: &mut BytesMut, ctx: ParseContext) -> ParseResult<RequestLine> {
        if buf.len() == 0 {
            return Ok(None);
        }
        // Unsafe: both headers_indices and headers are using unitialized memory,
        // but we *never* read any of it until after httparse has assigned
        // values into it. By not zeroing out the stack memory, this saves
        // a good ~5% on pipeline benchmarks.
        let mut headers_indices: [HeaderIndices; MAX_HEADERS] = unsafe { mem::uninitialized() };
        let (len, subject, version, headers_len) = {
            let mut headers: [httparse::Header; MAX_HEADERS] = unsafe { mem::uninitialized() };
            trace!("Request.parse([Header; {}], [u8; {}])", headers.len(), buf.len());
            let mut req = httparse::Request::new(&mut headers);
            match req.parse(&buf)? {
                httparse::Status::Complete(len) => {
                    trace!("Request.parse Complete({})", len);
                    let method = Method::from_bytes(req.method.unwrap().as_bytes())?;
                    let path = req.path.unwrap().parse()?;
                    let subject = RequestLine(method, path);
                    let version = if req.version.unwrap() == 1 {
                        Version::HTTP_11
                    } else {
                        Version::HTTP_10
                    };

                    record_header_indices(buf.as_ref(), &req.headers, &mut headers_indices);
                    let headers_len = req.headers.len();
                    (len, subject, version, headers_len)
                }
                httparse::Status::Partial => return Ok(None),
            }
        };

        let slice = buf.split_to(len).freeze();

        // According to https://tools.ietf.org/html/rfc7230#section-3.3.3
        // 1. (irrelevant to Request)
        // 2. (irrelevant to Request)
        // 3. Transfer-Encoding: chunked has a chunked body.
        // 4. If multiple differing Content-Length headers or invalid, close connection.
        // 5. Content-Length header has a sized body.
        // 6. Length 0.
        // 7. (irrelevant to Request)


        let mut decoder = None;
        let mut expect_continue = false;
        let mut keep_alive = version == Version::HTTP_11;
        let mut con_len = None;
        let mut is_te = false;
        let mut is_te_chunked = false;

        let mut headers = ctx.cached_headers
            .take()
            .unwrap_or_else(HeaderMap::new);

        headers.reserve(headers_len);

        for header in &headers_indices[..headers_len] {
            let name = HeaderName::from_bytes(&slice[header.name.0..header.name.1])
                .expect("header name already validated");
            let val = slice.slice(header.value.0, header.value.1);
            // Unsafe: httparse already validated header value
            let value = unsafe {
                HeaderValue::from_shared_unchecked(val)
            };

            match name {
                header::TRANSFER_ENCODING => {
                    // https://tools.ietf.org/html/rfc7230#section-3.3.3
                    // If Transfer-Encoding header is present, and 'chunked' is
                    // not the final encoding, and this is a Request, then it is
                    // mal-formed. A server should respond with 400 Bad Request.
                    if version == Version::HTTP_10 {
                        debug!("HTTP/1.0 cannot have Transfer-Encoding header");
                        return Err(Parse::Header);
                    }
                    is_te = true;
                    if headers::is_chunked_(&value) {
                        is_te_chunked = true;
                        decoder = Some(Decoder::chunked());
                        //debug!("request with transfer-encoding header, but not chunked, bad request");
                        //return Err(Parse::Header);
                    }
                },
                header::CONTENT_LENGTH => {
                    if is_te {
                        continue;
                    }
                    let len = value.to_str()
                        .map_err(|_| Parse::Header)
                        .and_then(|s| s.parse().map_err(|_| Parse::Header))?;
                    if let Some(prev) = con_len {
                        if prev != len {
                            debug!(
                                "multiple Content-Length headers with different values: [{}, {}]",
                                prev,
                                len,
                            );
                            return Err(Parse::Header);
                        }
                        // we don't need to append this secondary length
                        continue;
                    }
                    con_len = Some(len);
                    decoder = Some(Decoder::length(len));
                },
                header::CONNECTION => {
                    // keep_alive was previously set to default for Version
                    if keep_alive {
                        // HTTP/1.1
                        keep_alive = !headers::connection_close(&value);

                    } else {
                        // HTTP/1.0
                        keep_alive = headers::connection_keep_alive(&value);
                    }
                },
                header::EXPECT => {
                    expect_continue = value.as_bytes() == b"100-continue";
                },

                _ => (),
            }

            headers.append(name, value);
        }

        let decoder = if let Some(decoder) = decoder {
            decoder
        } else {
            if is_te && !is_te_chunked {
                debug!("request with transfer-encoding header, but not chunked, bad request");
                return Err(Parse::Header);
            }
            Decoder::length(0)
        };

        *ctx.req_method = Some(subject.0.clone());

        Ok(Some(ParsedMessage {
            head: MessageHead {
                version,
                subject,
                headers,
            },
            decode: Decode::Normal(decoder),
            expect_continue,
            keep_alive,
        }))
    }

    fn encode(mut msg: Encode<Self::Outgoing>, dst: &mut Vec<u8>) -> ::Result<Encoder> {
        trace!("Server::encode body={:?}, method={:?}", msg.body, msg.req_method);
        debug_assert!(!msg.title_case_headers, "no server config for title case headers");

        // hyper currently doesn't support returning 1xx status codes as a Response
        // This is because Service only allows returning a single Response, and
        // so if you try to reply with a e.g. 100 Continue, you have no way of
        // replying with the latter status code response.
        let (ret, mut is_last) = if StatusCode::SWITCHING_PROTOCOLS == msg.head.subject {
            (T::on_encode_upgrade(&mut msg), true)
        } else if msg.head.subject.is_informational() {
            error!("response with 1xx status code not supported");
            *msg.head = MessageHead::default();
            msg.head.subject = StatusCode::INTERNAL_SERVER_ERROR;
            msg.body = None;
            //TODO: change this to a more descriptive error than just a parse error
            (Err(::Error::new_status()), true)
        } else {
            (Ok(()), !msg.keep_alive)
        };

        // In some error cases, we don't know about the invalid message until already
        // pushing some bytes onto the `dst`. In those cases, we don't want to send
        // the half-pushed message, so rewind to before.
        let orig_len = dst.len();
        let rewind = |dst: &mut Vec<u8>| {
            dst.truncate(orig_len);
        };

        let init_cap = 30 + msg.head.headers.len() * AVERAGE_HEADER_SIZE;
        dst.reserve(init_cap);
        if msg.head.version == Version::HTTP_11 && msg.head.subject == StatusCode::OK {
            extend(dst, b"HTTP/1.1 200 OK\r\n");
        } else {
            match msg.head.version {
                Version::HTTP_10 => extend(dst, b"HTTP/1.0 "),
                Version::HTTP_11 => extend(dst, b"HTTP/1.1 "),
                _ => unreachable!(),
            }

            extend(dst, msg.head.subject.as_str().as_bytes());
            extend(dst, b" ");
            // a reason MUST be written, as many parsers will expect it.
            extend(dst, msg.head.subject.canonical_reason().unwrap_or("<none>").as_bytes());
            extend(dst, b"\r\n");
        }

        let mut encoder = Encoder::length(0);
        let mut wrote_len = false;
        let mut wrote_date = false;
        'headers: for (name, mut values) in msg.head.headers.drain() {
            match name {
                header::CONTENT_LENGTH => {
                    if wrote_len {
                        warn!("transfer-encoding and content-length both found, canceling");
                        rewind(dst);
                        return Err(::Error::new_header());
                    }
                    match msg.body {
                        Some(BodyLength::Known(len)) => {
                            // The Payload claims to know a length, and
                            // the headers are already set. For performance
                            // reasons, we are just going to trust that
                            // the values match.
                            //
                            // In debug builds, we'll assert they are the
                            // same to help developers find bugs.
                            encoder = Encoder::length(len);
                        },
                        Some(BodyLength::Unknown) => {
                            // The Payload impl didn't know how long the
                            // body is, but a length header was included.
                            // We have to parse the value to return our
                            // Encoder...
                            let mut folded = None::<(u64, HeaderValue)>;
                            for value in values {
                                if let Some(len) = headers::content_length_parse(&value) {
                                    if let Some(fold) = folded {
                                        if fold.0 != len {
                                            warn!("multiple Content-Length values found: [{}, {}]", fold.0, len);
                                            rewind(dst);
                                            return Err(::Error::new_header());
                                        }
                                        folded = Some(fold);
                                    } else {
                                        folded = Some((len, value));
                                    }
                                } else {
                                    warn!("illegal Content-Length value: {:?}", value);
                                    rewind(dst);
                                    return Err(::Error::new_header());
                                }
                            }
                            if let Some((len, value)) = folded {
                                encoder = Encoder::length(len);
                                extend(dst, b"content-length: ");
                                extend(dst, value.as_bytes());
                                extend(dst, b"\r\n");
                                wrote_len = true;
                                continue 'headers;
                            } else {
                                // No values in content-length... ignore?
                                continue 'headers;
                            }
                        },
                        None => {
                            // We have no body to actually send,
                            // but the headers claim a content-length.
                            // There's only 2 ways this makes sense:
                            //
                            // - The header says the length is `0`.
                            // - This is a response to a `HEAD` request.
                            if msg.req_method == &Some(Method::HEAD) {
                                debug_assert_eq!(encoder, Encoder::length(0));
                            } else {
                                for value in values {
                                    if value.as_bytes() != b"0" {
                                        warn!("content-length value found, but empty body provided: {:?}", value);
                                    }
                                }
                                continue 'headers;
                            }
                        }
                    }
                    wrote_len = true;
                },
                header::TRANSFER_ENCODING => {
                    if wrote_len {
                        warn!("transfer-encoding and content-length both found, canceling");
                        rewind(dst);
                        return Err(::Error::new_header());
                    }
                    // check that we actually can send a chunked body...
                    if msg.head.version == Version::HTTP_10 || !Server::can_chunked(msg.req_method, msg.head.subject) {
                        continue;
                    }
                    wrote_len = true;
                    encoder = Encoder::chunked();

                    extend(dst, b"transfer-encoding: ");

                    let mut saw_chunked;
                    if let Some(te) = values.next() {
                        extend(dst, te.as_bytes());
                        saw_chunked = headers::is_chunked_(&te);
                        for value in values {
                            extend(dst, b", ");
                            extend(dst, value.as_bytes());
                            saw_chunked = headers::is_chunked_(&value);
                        }
                        if !saw_chunked {
                            extend(dst, b", chunked\r\n");
                        } else {
                            extend(dst, b"\r\n");
                        }
                    } else {
                        // zero lines? add a chunked line then
                        extend(dst, b"chunked\r\n");
                    }
                    continue 'headers;
                },
                header::CONNECTION => {
                    if !is_last {
                        for value in values {
                            extend(dst, name.as_str().as_bytes());
                            extend(dst, b": ");
                            extend(dst, value.as_bytes());
                            extend(dst, b"\r\n");

                            if headers::connection_close(&value) {
                                is_last = true;
                            }
                        }
                        continue 'headers;
                    }
                },
                header::DATE => {
                    wrote_date = true;
                },
                _ => (),
            }
            //TODO: this should perhaps instead combine them into
            //single lines, as RFC7230 suggests is preferable.
            for value in values {
                extend(dst, name.as_str().as_bytes());
                extend(dst, b": ");
                extend(dst, value.as_bytes());
                extend(dst, b"\r\n");
            }
        }

        if !wrote_len {
            encoder = match msg.body {
                Some(BodyLength::Unknown) => {
                    if msg.head.version == Version::HTTP_10 || !Server::can_chunked(msg.req_method, msg.head.subject) {
                        Encoder::close_delimited()
                    } else {
                        extend(dst, b"transfer-encoding: chunked\r\n");
                        Encoder::chunked()
                    }
                },
                None |
                Some(BodyLength::Known(0)) => {
                    extend(dst, b"content-length: 0\r\n");
                    Encoder::length(0)
                },
                Some(BodyLength::Known(len)) => {
                    let _ = write!(FastWrite(dst), "content-length: {}\r\n", len);
                    Encoder::length(len)
                },
            };
        }

        // cached date is much faster than formatting every request
        if !wrote_date {
            dst.reserve(date::DATE_VALUE_LENGTH + 8);
            extend(dst, b"date: ");
            date::extend(dst);
            extend(dst, b"\r\n\r\n");
        } else {
            extend(dst, b"\r\n");
        }

        ret.map(|()| encoder.set_last(is_last))
    }

    fn on_error(err: &::Error) -> Option<MessageHead<Self::Outgoing>> {
        use ::error::{Kind, Parse};
        let status = match *err.kind() {
            Kind::Parse(Parse::Method) |
            Kind::Parse(Parse::Header) |
            Kind::Parse(Parse::Uri)    |
            Kind::Parse(Parse::Version) => {
                StatusCode::BAD_REQUEST
            },
            Kind::Parse(Parse::TooLarge) => {
                StatusCode::REQUEST_HEADER_FIELDS_TOO_LARGE
            },
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

    fn update_date() {
        date::update();
    }
}

impl Server<()> {
    /*
    fn set_length(head: &mut MessageHead<StatusCode>, body: Option<BodyLength>, method: Option<&Method>) -> Encoder {
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

        if let (Some(body), true) = (body, can_have_body) {
            set_length(&mut head.headers, body, head.version == Version::HTTP_11)
        } else {
            head.headers.remove(header::TRANSFER_ENCODING);
            if can_have_body {
                headers::content_length_zero(&mut head.headers);
            }
            Encoder::length(0)
        }
    }
    */

    fn can_chunked(method: &Option<Method>, status: StatusCode) -> bool {
        if method == &Some(Method::HEAD) {
            false
        } else if method == &Some(Method::CONNECT) && status.is_success() {
            false
        } else {
            match status {
                // TODO: support for 1xx codes needs improvement everywhere
                // would be 100...199 => false
                StatusCode::SWITCHING_PROTOCOLS |
                StatusCode::NO_CONTENT |
                StatusCode::NOT_MODIFIED => false,
                _ => true,
            }
        }
    }
}

impl<T> Http1Transaction for Client<T>
where
    T: OnUpgrade,
{
    type Incoming = StatusCode;
    type Outgoing = RequestLine;

    fn parse(buf: &mut BytesMut, ctx: ParseContext) -> ParseResult<StatusCode> {
        if buf.len() == 0 {
            return Ok(None);
        }
        // Unsafe: see comment in Server Http1Transaction, above.
        let mut headers_indices: [HeaderIndices; MAX_HEADERS] = unsafe { mem::uninitialized() };
        let (len, status, version, headers_len) = {
            let mut headers: [httparse::Header; MAX_HEADERS] = unsafe { mem::uninitialized() };
            trace!("Response.parse([Header; {}], [u8; {}])", headers.len(), buf.len());
            let mut res = httparse::Response::new(&mut headers);
            let bytes = buf.as_ref();
            match try!(res.parse(bytes)) {
                httparse::Status::Complete(len) => {
                    trace!("Response.parse Complete({})", len);
                    let status = StatusCode::from_u16(res.code.unwrap())?;
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

        let slice = buf.split_to(len).freeze();

        let mut headers = ctx.cached_headers
            .take()
            .unwrap_or_else(HeaderMap::new);

        headers.reserve(headers_len);
        fill_headers(&mut headers, slice, &headers_indices[..headers_len]);

        let keep_alive = version == Version::HTTP_11;

        let head = MessageHead {
            version,
            subject: status,
            headers,
        };
        let decode = Client::<T>::decoder(&head, ctx.req_method)?;

        Ok(Some(ParsedMessage {
            head,
            decode,
            expect_continue: false,
            keep_alive,
        }))
    }

    fn encode(msg: Encode<Self::Outgoing>, dst: &mut Vec<u8>) -> ::Result<Encoder> {
        trace!("Client::encode body={:?}, method={:?}", msg.body, msg.req_method);

        *msg.req_method = Some(msg.head.subject.0.clone());

        let body = Client::set_length(msg.head, msg.body);

        let init_cap = 30 + msg.head.headers.len() * AVERAGE_HEADER_SIZE;
        dst.reserve(init_cap);


        extend(dst, msg.head.subject.0.as_str().as_bytes());
        extend(dst, b" ");
        //TODO: add API to http::Uri to encode without std::fmt
        let _ = write!(FastWrite(dst), "{} ", msg.head.subject.1);

        match msg.head.version {
            Version::HTTP_10 => extend(dst, b"HTTP/1.0"),
            Version::HTTP_11 => extend(dst, b"HTTP/1.1"),
            _ => unreachable!(),
        }
        extend(dst, b"\r\n");

        if msg.title_case_headers {
            write_headers_title_case(&msg.head.headers, dst);
        } else {
            write_headers(&msg.head.headers, dst);
        }
        extend(dst, b"\r\n");
        msg.head.headers.clear(); //TODO: remove when switching to drain()

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

impl<T: OnUpgrade> Client<T> {
    fn decoder(inc: &MessageHead<StatusCode>, method: &mut Option<Method>) -> Result<Decode, Parse> {
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

        if inc.headers.contains_key(header::TRANSFER_ENCODING) {
            // https://tools.ietf.org/html/rfc7230#section-3.3.3
            // If Transfer-Encoding header is present, and 'chunked' is
            // not the final encoding, and this is a Request, then it is
            // mal-formed. A server should respond with 400 Bad Request.
            if inc.version == Version::HTTP_10 {
                debug!("HTTP/1.0 cannot have Transfer-Encoding header");
                Err(Parse::Header)
            } else if headers::transfer_encoding_is_chunked(&inc.headers) {
                Ok(Decode::Normal(Decoder::chunked()))
            } else {
                trace!("not chunked, read till eof");
                Ok(Decode::Normal(Decoder::eof()))
            }
        } else if let Some(len) = headers::content_length_parse_all(&inc.headers) {
            Ok(Decode::Normal(Decoder::length(len)))
        } else if inc.headers.contains_key(header::CONTENT_LENGTH) {
            debug!("illegal Content-Length header");
            Err(Parse::Header)
        } else {
            trace!("neither Transfer-Encoding nor Content-Length");
            Ok(Decode::Normal(Decoder::eof()))
        }
    }
}

impl Client<()> {
    fn set_length(head: &mut RequestHead, body: Option<BodyLength>) -> Encoder {
        if let Some(body) = body {
            let can_chunked = head.version == Version::HTTP_11
                && (head.subject.0 != Method::HEAD)
                && (head.subject.0 != Method::GET)
                && (head.subject.0 != Method::CONNECT);
            set_length(&mut head.headers, body, can_chunked)
        } else {
            head.headers.remove(header::TRANSFER_ENCODING);
            Encoder::length(0)
        }
    }
}

fn set_length(headers: &mut HeaderMap, body: BodyLength, can_chunked: bool) -> Encoder {
    // If the user already set specific headers, we should respect them, regardless
    // of what the Payload knows about itself. They set them for a reason.

    // Because of the borrow checker, we can't check the for an existing
    // Content-Length header while holding an `Entry` for the Transfer-Encoding
    // header, so unfortunately, we must do the check here, first.

    let existing_con_len = headers::content_length_parse_all(headers);
    let mut should_remove_con_len = false;

    if can_chunked {
        // If the user set a transfer-encoding, respect that. Let's just
        // make sure `chunked` is the final encoding.
        let encoder = match headers.entry(header::TRANSFER_ENCODING)
            .expect("TRANSFER_ENCODING is valid HeaderName") {
            Entry::Occupied(te) => {
                should_remove_con_len = true;
                if headers::is_chunked(te.iter()) {
                    Some(Encoder::chunked())
                } else {
                    warn!("user provided transfer-encoding does not end in 'chunked'");

                    // There's a Transfer-Encoding, but it doesn't end in 'chunked'!
                    // An example that could trigger this:
                    //
                    //     Transfer-Encoding: gzip
                    //
                    // This can be bad, depending on if this is a request or a
                    // response.
                    //
                    // - A request is illegal if there is a `Transfer-Encoding`
                    //   but it doesn't end in `chunked`.
                    // - A response that has `Transfer-Encoding` but doesn't
                    //   end in `chunked` isn't illegal, it just forces this
                    //   to be close-delimited.
                    //
                    // We can try to repair this, by adding `chunked` ourselves.

                    headers::add_chunked(te);
                    Some(Encoder::chunked())
                }
            },
            Entry::Vacant(te) => {
                if let Some(len) = existing_con_len {
                    Some(Encoder::length(len))
                } else if let BodyLength::Unknown = body {
                    should_remove_con_len = true;
                    te.insert(HeaderValue::from_static("chunked"));
                    Some(Encoder::chunked())
                } else {
                    None
                }
            },
        };

        // This is because we need a second mutable borrow to remove
        // content-length header.
        if let Some(encoder) = encoder {
            if should_remove_con_len && existing_con_len.is_some() {
                headers.remove(header::CONTENT_LENGTH);
            }
            return encoder;
        }

        // User didn't set transfer-encoding, AND we know body length,
        // so we can just set the Content-Length automatically.

        let len = if let BodyLength::Known(len) = body {
            len
        } else {
            unreachable!("BodyLength::Unknown would set chunked");
        };

        set_content_length(headers, len)
    } else {
        // Chunked isn't legal, so if it is set, we need to remove it.
        // Also, if it *is* set, then we shouldn't replace with a length,
        // since the user tried to imply there isn't a length.
        let encoder = if headers.remove(header::TRANSFER_ENCODING).is_some() {
            trace!("removing illegal transfer-encoding header");
            should_remove_con_len = true;
            Encoder::close_delimited()
        } else if let Some(len) = existing_con_len {
            Encoder::length(len)
        } else if let BodyLength::Known(len) = body {
            set_content_length(headers, len)
        } else {
            Encoder::close_delimited()
        };

        if should_remove_con_len && existing_con_len.is_some() {
            headers.remove(header::CONTENT_LENGTH);
        }

        encoder
    }
}

fn set_content_length(headers: &mut HeaderMap, len: u64) -> Encoder {
    // At this point, there should not be a valid Content-Length
    // header. However, since we'll be indexing in anyways, we can
    // warn the user if there was an existing illegal header.
    //
    // Or at least, we can in theory. It's actually a little bit slower,
    // so perhaps only do that while the user is developing/testing.

    if cfg!(debug_assertions) {
        match headers.entry(header::CONTENT_LENGTH)
            .expect("CONTENT_LENGTH is valid HeaderName") {
            Entry::Occupied(mut cl) => {
                // Internal sanity check, we should have already determined
                // that the header was illegal before calling this function.
                debug_assert!(headers::content_length_parse_all_values(cl.iter()).is_none());
                // Uh oh, the user set `Content-Length` headers, but set bad ones.
                // This would be an illegal message anyways, so let's try to repair
                // with our known good length.
                error!("user provided content-length header was invalid");

                cl.insert(headers::content_length_value(len));
                Encoder::length(len)
            },
            Entry::Vacant(cl) => {
                cl.insert(headers::content_length_value(len));
                Encoder::length(len)
            }
        }
    } else {
        headers.insert(header::CONTENT_LENGTH, headers::content_length_value(len));
        Encoder::length(len)
    }
}

pub(crate) trait OnUpgrade {
    fn on_encode_upgrade(msg: &mut Encode<StatusCode>) -> ::Result<()>;
    fn on_decode_upgrade() -> Result<Decoder, Parse>;
}

pub(crate) enum YesUpgrades {}

pub(crate) enum NoUpgrades {}

impl OnUpgrade for YesUpgrades {
    fn on_encode_upgrade(_: &mut Encode<StatusCode>) -> ::Result<()> {
        Ok(())
    }

    fn on_decode_upgrade() -> Result<Decoder, Parse> {
        debug!("101 response received, upgrading");
        // 101 upgrades always have no body
        Ok(Decoder::length(0))
    }
}

impl OnUpgrade for NoUpgrades {
    fn on_encode_upgrade(msg: &mut Encode<StatusCode>) -> ::Result<()> {
        error!("response with 101 status code not supported");
        *msg.head = MessageHead::default();
        msg.head.subject = ::StatusCode::INTERNAL_SERVER_ERROR;
        msg.body = None;
        //TODO: replace with more descriptive error
        Err(::Error::new_status())
    }

    fn on_decode_upgrade() -> Result<Decoder, Parse> {
        debug!("received 101 upgrade response, not supported");
        Err(Parse::UpgradeNotSupported)
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

fn fill_headers(headers: &mut HeaderMap, slice: Bytes, indices: &[HeaderIndices]) {
    for header in indices {
        let name = HeaderName::from_bytes(&slice[header.name.0..header.name.1])
            .expect("header name already validated");
        let value = unsafe {
            HeaderValue::from_shared_unchecked(
                slice.slice(header.value.0, header.value.1)
            )
        };
        headers.append(name, value);
    }
}

// Write header names as title case. The header name is assumed to be ASCII,
// therefore it is trivial to convert an ASCII character from lowercase to
// uppercase. It is as simple as XORing the lowercase character byte with
// space.
fn title_case(dst: &mut Vec<u8>, name: &[u8]) {
    dst.reserve(name.len());

    let mut iter = name.iter();

    // Uppercase the first character
    if let Some(c) = iter.next() {
        if *c >= b'a' && *c <= b'z' {
            dst.push(*c ^ b' ');
        }
    }

    while let Some(c) = iter.next() {
      dst.push(*c);

      if *c == b'-' {
          if let Some(c) = iter.next() {
              if *c >= b'a' && *c <= b'z' {
                  dst.push(*c ^ b' ');
              }
          }
      }
    }
}

fn write_headers_title_case(headers: &HeaderMap, dst: &mut Vec<u8>) {
    for (name, value) in headers {
        title_case(dst, name.as_str().as_bytes());
        extend(dst, b": ");
        extend(dst, value.as_bytes());
        extend(dst, b"\r\n");
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

    use super::*;
    use super::{Server as S, Client as C};

    type Server = S<NoUpgrades>;
    type Client = C<NoUpgrades>;

    #[test]
    fn test_parse_request() {
        extern crate pretty_env_logger;
        let _ = pretty_env_logger::try_init();
        let mut raw = BytesMut::from(b"GET /echo HTTP/1.1\r\nHost: hyper.rs\r\n\r\n".to_vec());
        let mut method = None;
        let msg = Server::parse(&mut raw, ParseContext {
            cached_headers: &mut None,
            req_method: &mut method,
        }).unwrap().unwrap();
        assert_eq!(raw.len(), 0);
        assert_eq!(msg.head.subject.0, ::Method::GET);
        assert_eq!(msg.head.subject.1, "/echo");
        assert_eq!(msg.head.version, ::Version::HTTP_11);
        assert_eq!(msg.head.headers.len(), 1);
        assert_eq!(msg.head.headers["Host"], "hyper.rs");
        assert_eq!(method, Some(::Method::GET));
    }


    #[test]
    fn test_parse_response() {
        extern crate pretty_env_logger;
        let _ = pretty_env_logger::try_init();
        let mut raw = BytesMut::from(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".to_vec());
        let ctx = ParseContext {
            cached_headers: &mut None,
            req_method: &mut Some(::Method::GET),
        };
        let msg = Client::parse(&mut raw, ctx).unwrap().unwrap();
        assert_eq!(raw.len(), 0);
        assert_eq!(msg.head.subject, ::StatusCode::OK);
        assert_eq!(msg.head.version, ::Version::HTTP_11);
        assert_eq!(msg.head.headers.len(), 1);
        assert_eq!(msg.head.headers["Content-Length"], "0");
    }

    #[test]
    fn test_parse_request_errors() {
        let mut raw = BytesMut::from(b"GET htt:p// HTTP/1.1\r\nHost: hyper.rs\r\n\r\n".to_vec());
        let ctx = ParseContext {
            cached_headers: &mut None,
            req_method: &mut None,
        };
        Server::parse(&mut raw, ctx).unwrap_err();
    }


    #[test]
    fn test_decoder_request() {
        use super::Decoder;

        fn parse(s: &str) -> ParsedMessage<RequestLine> {
            let mut bytes = BytesMut::from(s);
            Server::parse(&mut bytes, ParseContext {
                cached_headers: &mut None,
                req_method: &mut None,
            })
                .expect("parse ok")
                .expect("parse complete")
        }

        fn parse_err(s: &str, comment: &str) -> ::error::Parse {
            let mut bytes = BytesMut::from(s);
            Server::parse(&mut bytes, ParseContext {
                cached_headers: &mut None,
                req_method: &mut None,
            })
                .expect_err(comment)
        }

        // no length or transfer-encoding means 0-length body
        assert_eq!(parse("\
            GET / HTTP/1.1\r\n\
            \r\n\
        ").decode, Decode::Normal(Decoder::length(0)));

        assert_eq!(parse("\
            POST / HTTP/1.1\r\n\
            \r\n\
        ").decode, Decode::Normal(Decoder::length(0)));

        // transfer-encoding: chunked
        assert_eq!(parse("\
            POST / HTTP/1.1\r\n\
            transfer-encoding: chunked\r\n\
            \r\n\
        ").decode, Decode::Normal(Decoder::chunked()));

        assert_eq!(parse("\
            POST / HTTP/1.1\r\n\
            transfer-encoding: gzip, chunked\r\n\
            \r\n\
        ").decode, Decode::Normal(Decoder::chunked()));

        assert_eq!(parse("\
            POST / HTTP/1.1\r\n\
            transfer-encoding: gzip\r\n\
            transfer-encoding: chunked\r\n\
            \r\n\
        ").decode, Decode::Normal(Decoder::chunked()));

        // content-length
        assert_eq!(parse("\
            POST / HTTP/1.1\r\n\
            content-length: 10\r\n\
            \r\n\
        ").decode, Decode::Normal(Decoder::length(10)));

        // transfer-encoding and content-length = chunked
        assert_eq!(parse("\
            POST / HTTP/1.1\r\n\
            content-length: 10\r\n\
            transfer-encoding: chunked\r\n\
            \r\n\
        ").decode, Decode::Normal(Decoder::chunked()));

        assert_eq!(parse("\
            POST / HTTP/1.1\r\n\
            transfer-encoding: chunked\r\n\
            content-length: 10\r\n\
            \r\n\
        ").decode, Decode::Normal(Decoder::chunked()));

        assert_eq!(parse("\
            POST / HTTP/1.1\r\n\
            transfer-encoding: gzip\r\n\
            content-length: 10\r\n\
            transfer-encoding: chunked\r\n\
            \r\n\
        ").decode, Decode::Normal(Decoder::chunked()));


        // multiple content-lengths of same value are fine
        assert_eq!(parse("\
            POST / HTTP/1.1\r\n\
            content-length: 10\r\n\
            content-length: 10\r\n\
            \r\n\
        ").decode, Decode::Normal(Decoder::length(10)));


        // multiple content-lengths with different values is an error
        parse_err("\
            POST / HTTP/1.1\r\n\
            content-length: 10\r\n\
            content-length: 11\r\n\
            \r\n\
        ", "multiple content-lengths");

        // transfer-encoding that isn't chunked is an error
        parse_err("\
            POST / HTTP/1.1\r\n\
            transfer-encoding: gzip\r\n\
            \r\n\
        ", "transfer-encoding but not chunked");

        parse_err("\
            POST / HTTP/1.1\r\n\
            transfer-encoding: chunked, gzip\r\n\
            \r\n\
        ", "transfer-encoding doesn't end in chunked");


        // http/1.0

        assert_eq!(parse("\
            POST / HTTP/1.0\r\n\
            content-length: 10\r\n\
            \r\n\
        ").decode, Decode::Normal(Decoder::length(10)));


        // 1.0 doesn't understand chunked, so its an error
        parse_err("\
            POST / HTTP/1.0\r\n\
            transfer-encoding: chunked\r\n\
            \r\n\
        ", "1.0 chunked");
    }

    #[test]
    fn test_decoder_response() {

        fn parse(s: &str) -> ParsedMessage<StatusCode> {
            parse_with_method(s, Method::GET)
        }

        fn parse_with_method(s: &str, m: Method) -> ParsedMessage<StatusCode> {
            let mut bytes = BytesMut::from(s);
            Client::parse(&mut bytes, ParseContext {
                cached_headers: &mut None,
                req_method: &mut Some(m),
            })
                .expect("parse ok")
                .expect("parse complete")
        }

        fn parse_err(s: &str) -> ::error::Parse {
            let mut bytes = BytesMut::from(s);
            Client::parse(&mut bytes, ParseContext {
                cached_headers: &mut None,
                req_method: &mut Some(Method::GET),
            })
                .expect_err("parse should err")
        }


        // no content-length or transfer-encoding means close-delimited
        assert_eq!(parse("\
            HTTP/1.1 200 OK\r\n\
            \r\n\
        ").decode, Decode::Normal(Decoder::eof()));

        // 204 and 304 never have a body
        assert_eq!(parse("\
            HTTP/1.1 204 No Content\r\n\
            \r\n\
        ").decode, Decode::Normal(Decoder::length(0)));

        assert_eq!(parse("\
            HTTP/1.1 304 Not Modified\r\n\
            \r\n\
        ").decode, Decode::Normal(Decoder::length(0)));

        // content-length
        assert_eq!(parse("\
            HTTP/1.1 200 OK\r\n\
            content-length: 8\r\n\
            \r\n\
        ").decode, Decode::Normal(Decoder::length(8)));

        assert_eq!(parse("\
            HTTP/1.1 200 OK\r\n\
            content-length: 8\r\n\
            content-length: 8\r\n\
            \r\n\
        ").decode, Decode::Normal(Decoder::length(8)));

        parse_err("\
            HTTP/1.1 200 OK\r\n\
            content-length: 8\r\n\
            content-length: 9\r\n\
            \r\n\
        ");


        // transfer-encoding
        assert_eq!(parse("\
            HTTP/1.1 200 OK\r\n\
            transfer-encoding: chunked\r\n\
            \r\n\
        ").decode, Decode::Normal(Decoder::chunked()));

        // transfer-encoding and content-length = chunked
        assert_eq!(parse("\
            HTTP/1.1 200 OK\r\n\
            content-length: 10\r\n\
            transfer-encoding: chunked\r\n\
            \r\n\
        ").decode, Decode::Normal(Decoder::chunked()));


        // HEAD can have content-length, but not body
        assert_eq!(parse_with_method("\
            HTTP/1.1 200 OK\r\n\
            content-length: 8\r\n\
            \r\n\
        ", Method::HEAD).decode, Decode::Normal(Decoder::length(0)));

        // CONNECT with 200 never has body
        assert_eq!(parse_with_method("\
            HTTP/1.1 200 OK\r\n\
            \r\n\
        ", Method::CONNECT).decode, Decode::Final(Decoder::length(0)));

        // CONNECT receiving non 200 can have a body
        assert_eq!(parse_with_method("\
            HTTP/1.1 400 Bad Request\r\n\
            \r\n\
        ", Method::CONNECT).decode, Decode::Normal(Decoder::eof()));


        // 1xx status codes
        assert_eq!(parse("\
            HTTP/1.1 100 Continue\r\n\
            \r\n\
        ").decode, Decode::Ignore);

        assert_eq!(parse("\
            HTTP/1.1 103 Early Hints\r\n\
            \r\n\
        ").decode, Decode::Ignore);

        // 101 upgrade not supported yet
        parse_err("\
            HTTP/1.1 101 Switching Protocols\r\n\
            \r\n\
        ");


        // http/1.0
        assert_eq!(parse("\
            HTTP/1.0 200 OK\r\n\
            \r\n\
        ").decode, Decode::Normal(Decoder::eof()));

        // 1.0 doesn't understand chunked
        parse_err("\
            HTTP/1.0 200 OK\r\n\
            transfer-encoding: chunked\r\n\
            \r\n\
        ");
    }

    #[test]
    fn test_client_request_encode_title_case() {
        use http::header::HeaderValue;
        use proto::BodyLength;

        let mut head = MessageHead::default();
        head.headers.insert("content-length", HeaderValue::from_static("10"));
        head.headers.insert("content-type", HeaderValue::from_static("application/json"));

        let mut vec = Vec::new();
        Client::encode(Encode {
            head: &mut head,
            body: Some(BodyLength::Known(10)),
            keep_alive: true,
            req_method: &mut None,
            title_case_headers: true,
        }, &mut vec).unwrap();

        assert_eq!(vec, b"GET / HTTP/1.1\r\nContent-Length: 10\r\nContent-Type: application/json\r\n\r\n".to_vec());
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
        let mut headers = Some(HeaderMap::new());

        b.bytes = len as u64;
        b.iter(|| {
            let msg = Server::parse(&mut raw, ParseContext {
                cached_headers: &mut headers,
                req_method: &mut None,
            }).unwrap().unwrap();
            headers = Some(msg.head.headers);
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
    fn bench_parse_short(b: &mut Bencher) {
        let mut raw = BytesMut::from(
            b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n".to_vec()
        );
        let len = raw.len();
        let mut headers = Some(HeaderMap::new());

        b.bytes = len as u64;
        b.iter(|| {
            let msg = Server::parse(&mut raw, ParseContext {
                cached_headers: &mut headers,
                req_method: &mut None,
            }).unwrap().unwrap();
            headers = Some(msg.head.headers);
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
    fn bench_server_encode_headers_preset(b: &mut Bencher) {
        use http::header::HeaderValue;
        use proto::BodyLength;

        let len = 108;
        b.bytes = len as u64;

        let mut head = MessageHead::default();
        let mut headers = HeaderMap::new();
        headers.insert("content-length", HeaderValue::from_static("10"));
        headers.insert("content-type", HeaderValue::from_static("application/json"));

        b.iter(|| {
            let mut vec = Vec::new();
            head.headers = headers.clone();
            Server::encode(Encode {
                head: &mut head,
                body: Some(BodyLength::Known(10)),
                keep_alive: true,
                req_method: &mut Some(Method::GET),
                title_case_headers: false,
            }, &mut vec).unwrap();
            assert_eq!(vec.len(), len);
            ::test::black_box(vec);
        })
    }

    #[cfg(feature = "nightly")]
    #[bench]
    fn bench_server_encode_no_headers(b: &mut Bencher) {
        use proto::BodyLength;

        let len = 76;
        b.bytes = len as u64;

        let mut head = MessageHead::default();

        b.iter(|| {
            let mut vec = Vec::new();
            Server::encode(Encode {
                head: &mut head,
                body: Some(BodyLength::Known(10)),
                keep_alive: true,
                req_method: &mut Some(Method::GET),
                title_case_headers: false,
            }, &mut vec).unwrap();
            assert_eq!(vec.len(), len);
            ::test::black_box(vec);
        })
    }
}
