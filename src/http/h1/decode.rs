use std::usize;
use std::io;

use bytes::Bytes;
use http::io::MemRead;

use self::Kind::{Length, Chunked, Eof};

/// Decoders to handle different Transfer-Encodings.
///
/// If a message body does not include a Transfer-Encoding, it *should*
/// include a Content-Length header.
#[derive(Debug, Clone)]
pub struct Decoder {
    kind: Kind,
}

impl Decoder {
    pub fn length(x: u64) -> Decoder {
        Decoder { kind: Kind::Length(x) }
    }

    pub fn chunked() -> Decoder {
        Decoder { kind: Kind::Chunked(ChunkedState::Size, 0) }
    }

    pub fn eof() -> Decoder {
        Decoder { kind: Kind::Eof(false) }
    }
}

#[derive(Debug, Clone)]
enum Kind {
    /// A Reader used when a Content-Length header is passed with a positive integer.
    Length(u64),
    /// A Reader used when Transfer-Encoding is `chunked`.
    Chunked(ChunkedState, u64),
    /// A Reader used for responses that don't indicate a length or chunked.
    ///
    /// Note: This should only used for `Response`s. It is illegal for a
    /// `Request` to be made with both `Content-Length` and
    /// `Transfer-Encoding: chunked` missing, as explained from the spec:
    ///
    /// > If a Transfer-Encoding header field is present in a response and
    /// > the chunked transfer coding is not the final encoding, the
    /// > message body length is determined by reading the connection until
    /// > it is closed by the server.  If a Transfer-Encoding header field
    /// > is present in a request and the chunked transfer coding is not
    /// > the final encoding, the message body length cannot be determined
    /// > reliably; the server MUST respond with the 400 (Bad Request)
    /// > status code and then close the connection.
    Eof(bool),
}

#[derive(Debug, PartialEq, Clone)]
enum ChunkedState {
    Size,
    SizeLws,
    Extension,
    SizeLf,
    Body,
    BodyCr,
    BodyLf,
    EndCr,
    EndLf,
    End,
}

impl Decoder {
    pub fn is_eof(&self) -> bool {
        trace!("is_eof? {:?}", self);
        match self.kind {
            Length(0) |
            Chunked(ChunkedState::End, _) |
            Eof(true) => true,
            _ => false,
        }
    }
}

impl Decoder {
    pub fn decode<R: MemRead>(&mut self, body: &mut R) -> io::Result<Bytes> {
        match self.kind {
            Length(ref mut remaining) => {
                trace!("Sized read, remaining={:?}", remaining);
                if *remaining == 0 {
                    Ok(Bytes::new())
                } else {
                    let to_read = *remaining as usize;
                    let buf = try!(body.read_mem(to_read));
                    let num = buf.as_ref().len() as u64;
                    trace!("Length read: {}", num);
                    if num > *remaining {
                        *remaining = 0;
                    } else if num == 0 {
                        return Err(io::Error::new(io::ErrorKind::Other, "early eof"));
                    } else {
                        *remaining -= num;
                    }
                    Ok(buf)
                }
            }
            Chunked(ref mut state, ref mut size) => {
                loop {
                    let mut buf = None;
                    // advances the chunked state
                    *state = try!(state.step(body, size, &mut buf));
                    if *state == ChunkedState::End {
                        trace!("end of chunked");
                        return Ok(Bytes::new());
                    }
                    if let Some(buf) = buf {
                        return Ok(buf);
                    }
                }
            }
            Eof(ref mut is_eof) => {
                if *is_eof {
                    Ok(Bytes::new())
                } else {
                    // 8192 chosen because its about 2 packets, there probably
                    // won't be that much available, so don't have MemReaders
                    // allocate buffers to big
                    match body.read_mem(8192) {
                        Ok(slice) => {
                            *is_eof = slice.is_empty();
                            Ok(slice)
                        }
                        other => other,
                    }
                }
            }
        }
    }
}

macro_rules! byte (
    ($rdr:ident) => ({
        let buf = try!($rdr.read_mem(1));
        if !buf.is_empty() {
            buf[0]
        } else {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof,
                                      "Unexpected eof during chunk size line"));
        }
    })
);

impl ChunkedState {
    fn step<R: MemRead>(&self,
                        body: &mut R,
                        size: &mut u64,
                        buf: &mut Option<Bytes>)
                        -> io::Result<ChunkedState> {
        use self::ChunkedState::*;
        Ok(match *self {
            Size => try!(ChunkedState::read_size(body, size)),
            SizeLws => try!(ChunkedState::read_size_lws(body)),
            Extension => try!(ChunkedState::read_extension(body)),
            SizeLf => try!(ChunkedState::read_size_lf(body, size)),
            Body => try!(ChunkedState::read_body(body, size, buf)),
            BodyCr => try!(ChunkedState::read_body_cr(body)),
            BodyLf => try!(ChunkedState::read_body_lf(body)),
            EndCr => try!(ChunkedState::read_end_cr(body)),
            EndLf => try!(ChunkedState::read_end_lf(body)),
            End => ChunkedState::End,
        })
    }
    fn read_size<R: MemRead>(rdr: &mut R, size: &mut u64) -> io::Result<ChunkedState> {
        trace!("Read chunk hex size");
        let radix = 16;
        match byte!(rdr) {
            b @ b'0'...b'9' => {
                *size *= radix;
                *size += (b - b'0') as u64;
            }
            b @ b'a'...b'f' => {
                *size *= radix;
                *size += (b + 10 - b'a') as u64;
            }
            b @ b'A'...b'F' => {
                *size *= radix;
                *size += (b + 10 - b'A') as u64;
            }
            b'\t' | b' ' => return Ok(ChunkedState::SizeLws),
            b';' => return Ok(ChunkedState::Extension),
            b'\r' => return Ok(ChunkedState::SizeLf),
            _ => {
                return Err(io::Error::new(io::ErrorKind::InvalidInput,
                                          "Invalid chunk size line: Invalid Size"));
            }
        }
        Ok(ChunkedState::Size)
    }
    fn read_size_lws<R: MemRead>(rdr: &mut R) -> io::Result<ChunkedState> {
        trace!("read_size_lws");
        match byte!(rdr) {
            // LWS can follow the chunk size, but no more digits can come
            b'\t' | b' ' => Ok(ChunkedState::SizeLws),
            b';' => Ok(ChunkedState::Extension),
            b'\r' => return Ok(ChunkedState::SizeLf),
            _ => {
                Err(io::Error::new(io::ErrorKind::InvalidInput,
                                   "Invalid chunk size linear white space"))
            }
        }
    }
    fn read_extension<R: MemRead>(rdr: &mut R) -> io::Result<ChunkedState> {
        trace!("read_extension");
        match byte!(rdr) {
            b'\r' => return Ok(ChunkedState::SizeLf),
            _ => return Ok(ChunkedState::Extension), // no supported extensions
        }
    }
    fn read_size_lf<R: MemRead>(rdr: &mut R, size: &mut u64) -> io::Result<ChunkedState> {
        trace!("Chunk size is {:?}", size);
        match byte!(rdr) {
            b'\n' if *size > 0 => Ok(ChunkedState::Body),
            b'\n' if *size == 0 => Ok(ChunkedState::EndCr),
            _ => Err(io::Error::new(io::ErrorKind::InvalidInput, "Invalid chunk size LF")),
        }
    }

    fn read_body<R: MemRead>(rdr: &mut R,
                          rem: &mut u64,
                          buf: &mut Option<Bytes>)
                          -> io::Result<ChunkedState> {
        trace!("Chunked read, remaining={:?}", rem);

        // cap remaining bytes at the max capacity of usize
        let rem_cap = match *rem {
            r if r > usize::MAX as u64 => usize::MAX,
            r => r as usize,
        };

        let to_read = rem_cap;
        let slice = try!(rdr.read_mem(to_read));
        let count = slice.len();

        if count == 0 {
            *rem = 0;
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "early eof"));
        }
        *buf = Some(slice);
        *rem -= count as u64;

        if *rem > 0 {
            Ok(ChunkedState::Body)
        } else {
            Ok(ChunkedState::BodyCr)
        }
    }
    fn read_body_cr<R: MemRead>(rdr: &mut R) -> io::Result<ChunkedState> {
        match byte!(rdr) {
            b'\r' => Ok(ChunkedState::BodyLf),
            _ => Err(io::Error::new(io::ErrorKind::InvalidInput, "Invalid chunk body CR")),
        }
    }
    fn read_body_lf<R: MemRead>(rdr: &mut R) -> io::Result<ChunkedState> {
        match byte!(rdr) {
            b'\n' => Ok(ChunkedState::Size),
            _ => Err(io::Error::new(io::ErrorKind::InvalidInput, "Invalid chunk body LF")),
        }
    }

    fn read_end_cr<R: MemRead>(rdr: &mut R) -> io::Result<ChunkedState> {
        match byte!(rdr) {
            b'\r' => Ok(ChunkedState::EndLf),
            _ => Err(io::Error::new(io::ErrorKind::InvalidInput, "Invalid chunk end CR")),
        }
    }
    fn read_end_lf<R: MemRead>(rdr: &mut R) -> io::Result<ChunkedState> {
        match byte!(rdr) {
            b'\n' => Ok(ChunkedState::End),
            _ => Err(io::Error::new(io::ErrorKind::InvalidInput, "Invalid chunk end LF")),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::io;
    use std::io::Write;
    use super::Decoder;
    use super::ChunkedState;
    use http::io::MemRead;
    use bytes::{BytesMut, Bytes};
    use mock::AsyncIo;

    impl<'a> MemRead for &'a [u8] {
        fn read_mem(&mut self, len: usize) -> io::Result<Bytes> {
            let n = ::std::cmp::min(len, self.len());
            if n > 0 {
                let (a, b) = self.split_at(n);
                let mut buf = BytesMut::from(a);
                *self = b;
                Ok(buf.drain_to(n).freeze())
            } else {
                Ok(Bytes::new())
            }
        }
    }

    #[test]
    fn test_read_chunk_size() {
        use std::io::ErrorKind::{UnexpectedEof, InvalidInput};

        fn read(s: &str) -> u64 {
            let mut state = ChunkedState::Size;
            let mut rdr = &mut s.as_bytes();
            let mut size = 0;
            loop {
                let result = state.step(rdr, &mut size, &mut None);
                let desc = format!("read_size failed for {:?}", s);
                state = result.expect(desc.as_str());
                if state == ChunkedState::Body || state == ChunkedState::EndCr {
                    break;
                }
            }
            size
        }

        fn read_err(s: &str, expected_err: io::ErrorKind) {
            let mut state = ChunkedState::Size;
            let mut rdr = &mut s.as_bytes();
            let mut size = 0;
            loop {
                let result = state.step(rdr, &mut size, &mut None);
                state = match result {
                    Ok(s) => s,
                    Err(e) => {
                        assert!(expected_err == e.kind(), "Reading {:?}, expected {:?}, but got {:?}",
                                                          s, expected_err, e.kind());
                        return;
                    }
                };
                if state == ChunkedState::Body || state == ChunkedState::End {
                    panic!(format!("Was Ok. Expected Err for {:?}", s));
                }
            }
        }

        assert_eq!(1, read("1\r\n"));
        assert_eq!(1, read("01\r\n"));
        assert_eq!(0, read("0\r\n"));
        assert_eq!(0, read("00\r\n"));
        assert_eq!(10, read("A\r\n"));
        assert_eq!(10, read("a\r\n"));
        assert_eq!(255, read("Ff\r\n"));
        assert_eq!(255, read("Ff   \r\n"));
        // Missing LF or CRLF
        read_err("F\rF", InvalidInput);
        read_err("F", UnexpectedEof);
        // Invalid hex digit
        read_err("X\r\n", InvalidInput);
        read_err("1X\r\n", InvalidInput);
        read_err("-\r\n", InvalidInput);
        read_err("-1\r\n", InvalidInput);
        // Acceptable (if not fully valid) extensions do not influence the size
        assert_eq!(1, read("1;extension\r\n"));
        assert_eq!(10, read("a;ext name=value\r\n"));
        assert_eq!(1, read("1;extension;extension2\r\n"));
        assert_eq!(1, read("1;;;  ;\r\n"));
        assert_eq!(2, read("2; extension...\r\n"));
        assert_eq!(3, read("3   ; extension=123\r\n"));
        assert_eq!(3, read("3   ;\r\n"));
        assert_eq!(3, read("3   ;   \r\n"));
        // Invalid extensions cause an error
        read_err("1 invalid extension\r\n", InvalidInput);
        read_err("1 A\r\n", InvalidInput);
        read_err("1;no CRLF", UnexpectedEof);
    }

    #[test]
    fn test_read_sized_early_eof() {
        let mut bytes = &b"foo bar"[..];
        let mut decoder = Decoder::length(10);
        assert_eq!(decoder.decode(&mut bytes).unwrap().len(), 7);
        let e = decoder.decode(&mut bytes).unwrap_err();
        assert_eq!(e.kind(), io::ErrorKind::Other);
        assert_eq!(e.description(), "early eof");
    }

    #[test]
    fn test_read_chunked_early_eof() {
        let mut bytes = &b"\
            9\r\n\
            foo bar\
        "[..];
        let mut decoder = Decoder::chunked();
        assert_eq!(decoder.decode(&mut bytes).unwrap().len(), 7);
        let e = decoder.decode(&mut bytes).unwrap_err();
        assert_eq!(e.kind(), io::ErrorKind::UnexpectedEof);
        assert_eq!(e.description(), "early eof");
    }

    #[test]
    fn test_read_chunked_single_read() {
        let mut mock_buf = &b"10\r\n1234567890abcdef\r\n0\r\n"[..];
        let buf = Decoder::chunked().decode(&mut mock_buf).expect("decode");
        assert_eq!(16, buf.len());
        let result = String::from_utf8(buf.as_ref().to_vec()).expect("decode String");
        assert_eq!("1234567890abcdef", &result);
    }

    #[test]
    fn test_read_chunked_after_eof() {
        let mut mock_buf = &b"10\r\n1234567890abcdef\r\n0\r\n\r\n"[..];
        let mut decoder = Decoder::chunked();

        // normal read
        let buf = decoder.decode(&mut mock_buf).expect("decode");
        assert_eq!(16, buf.len());
        let result = String::from_utf8(buf.as_ref().to_vec()).expect("decode String");
        assert_eq!("1234567890abcdef", &result);

        // eof read
        let buf = decoder.decode(&mut mock_buf).expect("decode");
        assert_eq!(0, buf.len());

        // ensure read after eof also returns eof
        let buf = decoder.decode(&mut mock_buf).expect("decode");
        assert_eq!(0, buf.len());
    }

    // perform an async read using a custom buffer size and causing a blocking
    // read at the specified byte
    fn read_async(mut decoder: Decoder,
                  content: &[u8],
                  block_at: usize)
                  -> String {
        let content_len = content.len();
        let mut ins = AsyncIo::new(content, block_at);
        let mut outs = Vec::new();
        loop {
            match decoder.decode(&mut ins) {
                Ok(buf) => {
                    if buf.is_empty() {
                        break; // eof
                    }
                    outs.write(buf.as_ref()).expect("write buffer");
                }
                Err(e) => match e.kind() {
                    io::ErrorKind::WouldBlock => {
                        ins.block_in(content_len); // we only block once
                    },
                    _ => panic!("unexpected decode error: {}", e),
                }
            };
        }
        String::from_utf8(outs).expect("decode String")
    }

    // iterate over the different ways that this async read could go.
    // tests blocking a read at each byte along the content - The shotgun approach
    fn all_async_cases(content: &str, expected: &str, decoder: Decoder) {
        let content_len = content.len();
        for block_at in 0..content_len {
            let actual = read_async(decoder.clone(), content.as_bytes(), block_at);
            assert_eq!(expected, &actual, "Failed async. Blocking at {}", block_at);
        }
    }

    #[test]
    fn test_read_length_async() {
        let content = "foobar";
        all_async_cases(content, content, Decoder::length(content.len() as u64));
    }

    #[test]
    fn test_read_chunked_async() {
        let content = "3\r\nfoo\r\n3\r\nbar\r\n0\r\n\r\n";
        let expected = "foobar";
        all_async_cases(content, expected, Decoder::chunked());
    }

    #[test]
    fn test_read_eof_async() {
        let content = "foobar";
        all_async_cases(content, content, Decoder::eof());
    }

}
