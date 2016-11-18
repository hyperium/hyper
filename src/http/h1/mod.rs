pub use self::decode::Decoder;
pub use self::encode::Encoder;

pub use self::parse::parse;

mod decode;
mod encode;
pub mod parse;

/*
fn should_have_response_body(method: &Method, status: u16) -> bool {
    trace!("should_have_response_body({:?}, {})", method, status);
    match (method, status) {
        (&Method::Head, _) |
        (_, 100...199) |
        (_, 204) |
        (_, 304) |
        (&Method::Connect, 200...299) => false,
        _ => true
    }
}
*/
/*
const MAX_INVALID_RESPONSE_BYTES: usize = 1024 * 128;
impl HttpMessage for Http11Message {

    fn get_incoming(&mut self) -> ::Result<ResponseHead> {
        unimplemented!();
        /*
        try!(self.flush_outgoing());
        let stream = match self.stream.take() {
            Some(stream) => stream,
            None => {
                // The message was already in the reading state...
                // TODO Decide what happens in case we try to get a new incoming at that point
                return Err(From::from(
                        io::Error::new(io::ErrorKind::Other,
                        "Read already in progress")));
            }
        };

        let expected_no_content = stream.previous_response_expected_no_content();
        trace!("previous_response_expected_no_content = {}", expected_no_content);

        let mut stream = BufReader::new(stream);

        let mut invalid_bytes_read = 0;
        let head;
        loop {
            head = match parse_response(&mut stream) {
                Ok(head) => head,
                Err(::Error::Version)
                if expected_no_content && invalid_bytes_read < MAX_INVALID_RESPONSE_BYTES => {
                    trace!("expected_no_content, found content");
                    invalid_bytes_read += 1;
                    stream.consume(1);
                    continue;
                }
                Err(e) => {
                    self.stream = Some(stream.into_inner());
                    return Err(e);
                }
            };
            break;
        }

        let raw_status = head.subject;
        let headers = head.headers;

        let method = self.method.take().unwrap_or(Method::Get);

        let is_empty = !should_have_response_body(&method, raw_status.0);
        stream.get_mut().set_previous_response_expected_no_content(is_empty);
        // According to https://tools.ietf.org/html/rfc7230#section-3.3.3
        // 1. HEAD reponses, and Status 1xx, 204, and 304 cannot have a body.
        // 2. Status 2xx to a CONNECT cannot have a body.
        // 3. Transfer-Encoding: chunked has a chunked body.
        // 4. If multiple differing Content-Length headers or invalid, close connection.
        // 5. Content-Length header has a sized body.
        // 6. Not Client.
        // 7. Read till EOF.
        self.reader = Some(if is_empty {
            SizedReader(stream, 0)
        } else {
             if let Some(&TransferEncoding(ref codings)) = headers.get() {
                if codings.last() == Some(&Chunked) {
                    ChunkedReader(stream, None)
                } else {
                    trace!("not chuncked. read till eof");
                    EofReader(stream)
                }
            } else if let Some(&ContentLength(len)) =  headers.get() {
                SizedReader(stream, len)
            } else if headers.has::<ContentLength>() {
                trace!("illegal Content-Length: {:?}", headers.get_raw("Content-Length"));
                return Err(Error::Header);
            } else {
                trace!("neither Transfer-Encoding nor Content-Length");
                EofReader(stream)
            }
        });

        trace!("Http11Message.reader = {:?}", self.reader);


        Ok(ResponseHead {
            headers: headers,
            raw_status: raw_status,
            version: head.version,
        })
        */
    }
}


*/


