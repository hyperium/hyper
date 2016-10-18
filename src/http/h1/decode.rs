use std::{cmp, usize};
use std::io::{self, Read};

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
    pub fn decode<R: Read>(&mut self, body: &mut R, buf: &mut [u8]) -> io::Result<usize> {
        match self.kind {
            Length(ref mut remaining) => {
                trace!("Sized read, remaining={:?}", remaining);
                if *remaining == 0 {
                    Ok(0)
                } else {
                    let to_read = cmp::min(*remaining as usize, buf.len());
                    let num = try!(body.read(&mut buf[..to_read])) as u64;
                    trace!("Length read: {}", num);
                    if num > *remaining {
                        *remaining = 0;
                    } else if num == 0 {
                        return Err(io::Error::new(io::ErrorKind::Other, "early eof"));
                    } else {
                        *remaining -= num;
                    }
                    Ok(num as usize)
                }
            }
            Chunked(ref mut state, ref mut size) => {
                loop {
                    let mut read = 0;
                    // advances the chunked state
                    *state = try!(state.step(body, size, buf, &mut read));
                    if *state == ChunkedState::End {
                        trace!("end of chunked");
                        return Ok(0);
                    }
                    if read > 0 {
                        return Ok(read);
                    }
                }
            }
            Eof(ref mut is_eof) => {
                match body.read(buf) {
                    Ok(0) => {
                        *is_eof = true;
                        Ok(0)
                    }
                    other => other,
                }
            }
        }
    }
}

macro_rules! byte (
    ($rdr:ident) => ({
        let mut buf = [0];
        match try!($rdr.read(&mut buf)) {
            1 => buf[0],
            _ => return Err(io::Error::new(io::ErrorKind::UnexpectedEof,
                                           "Unexpected eof during chunk size line")),
        }
    })
);

impl ChunkedState {
    fn step<R: Read>(&self,
                     body: &mut R,
                     size: &mut u64,
                     buf: &mut [u8],
                     read: &mut usize)
                     -> io::Result<ChunkedState> {
        use self::ChunkedState::*;
        Ok(match *self {
            Size => try!(ChunkedState::read_size(body, size)),
            SizeLws => try!(ChunkedState::read_size_lws(body)),
            Extension => try!(ChunkedState::read_extension(body)),
            SizeLf => try!(ChunkedState::read_size_lf(body, size)),
            Body => try!(ChunkedState::read_body(body, size, buf, read)),
            BodyCr => try!(ChunkedState::read_body_cr(body)),
            BodyLf => try!(ChunkedState::read_body_lf(body)),
            EndCr => try!(ChunkedState::read_end_cr(body)),
            EndLf => try!(ChunkedState::read_end_lf(body)),
            End => ChunkedState::End,
        })
    }
    fn read_size<R: Read>(rdr: &mut R, size: &mut u64) -> io::Result<ChunkedState> {
        trace!("Read size");
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
    fn read_size_lws<R: Read>(rdr: &mut R) -> io::Result<ChunkedState> {
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
    fn read_extension<R: Read>(rdr: &mut R) -> io::Result<ChunkedState> {
        trace!("read_extension");
        match byte!(rdr) {
            b'\r' => return Ok(ChunkedState::SizeLf),
            _ => return Ok(ChunkedState::Extension), // no supported extensions
        }
    }
    fn read_size_lf<R: Read>(rdr: &mut R, size: &mut u64) -> io::Result<ChunkedState> {
        trace!("Chunk size is {:?}", size);
        match byte!(rdr) {
            b'\n' if *size > 0 => Ok(ChunkedState::Body),
            b'\n' if *size == 0 => Ok(ChunkedState::EndCr),
            _ => Err(io::Error::new(io::ErrorKind::InvalidInput, "Invalid chunk size LF")),
        }
    }

    fn read_body<R: Read>(rdr: &mut R,
                          rem: &mut u64,
                          buf: &mut [u8],
                          read: &mut usize)
                          -> io::Result<ChunkedState> {
        trace!("Chunked read, remaining={:?}", rem);

        // cap remaining bytes at the max capacity of usize
        let rem_cap = match *rem {
            r if r > usize::MAX as u64 => usize::MAX,
            r => r as usize,
        };

        let to_read = cmp::min(rem_cap, buf.len());
        let count = try!(rdr.read(&mut buf[..to_read]));

        trace!("to_read = {}", to_read);
        trace!("count = {}", count);

        if count == 0 {
            *rem = 0;
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "early eof"));
        }

        *rem -= count as u64;
        *read = count;

        if *rem > 0 {
            Ok(ChunkedState::Body)
        } else {
            Ok(ChunkedState::BodyCr)
        }
    }
    fn read_body_cr<R: Read>(rdr: &mut R) -> io::Result<ChunkedState> {
        match byte!(rdr) {
            b'\r' => Ok(ChunkedState::BodyLf),
            _ => Err(io::Error::new(io::ErrorKind::InvalidInput, "Invalid chunk body CR")),
        }
    }
    fn read_body_lf<R: Read>(rdr: &mut R) -> io::Result<ChunkedState> {
        match byte!(rdr) {
            b'\n' => Ok(ChunkedState::Size),
            _ => Err(io::Error::new(io::ErrorKind::InvalidInput, "Invalid chunk body LF")),
        }
    }

    fn read_end_cr<R: Read>(rdr: &mut R) -> io::Result<ChunkedState> {
        match byte!(rdr) {
            b'\r' => Ok(ChunkedState::EndLf),
            _ => Err(io::Error::new(io::ErrorKind::InvalidInput, "Invalid chunk end CR")),
        }
    }
    fn read_end_lf<R: Read>(rdr: &mut R) -> io::Result<ChunkedState> {
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
    use mock::Async;

    #[test]
    fn test_read_chunk_size() {
        use std::io::ErrorKind::{UnexpectedEof, InvalidInput};

        fn read(s: &str) -> u64 {
            let mut state = ChunkedState::Size;
            let mut rdr = &mut s.as_bytes();
            let mut size = 0;
            let mut count = 0;
            loop {
                let mut buf = [0u8; 10];
                let result = state.step(&mut rdr, &mut size, &mut buf, &mut count);
                let desc = format!("read_size failed for {:?}", s);
                state = result.expect(desc.as_str());
                trace!("State {:?}", state);
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
            let mut count = 0;
            loop {
                let mut buf = [0u8; 10];
                let result = state.step(&mut rdr, &mut size, &mut buf, &mut count);
                state = match result {
                    Ok(s) => s,
                    Err(e) => {
                        assert!(expected_err == e.kind(), "Reading {:?}, expected {:?}, but got {:?}",
                                                          s, expected_err, e.kind());
                        return;
                    }
                };
                trace!("State {:?}", state);
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
        let mut buf = [0u8; 10];
        assert_eq!(decoder.decode(&mut bytes, &mut buf).unwrap(), 7);
        let e = decoder.decode(&mut bytes, &mut buf).unwrap_err();
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
        let mut buf = [0u8; 10];
        assert_eq!(decoder.decode(&mut bytes, &mut buf).unwrap(), 7);
        let e = decoder.decode(&mut bytes, &mut buf).unwrap_err();
        assert_eq!(e.kind(), io::ErrorKind::UnexpectedEof);
        assert_eq!(e.description(), "early eof");
    }

    #[test]
    fn test_read_chunked_single_read() {
        let content = b"10\r\n1234567890abcdef\r\n0\r\n";
        let mut mock_buf = io::Cursor::new(content);
        let mut buf = [0u8; 16];
        let count = Decoder::chunked().decode(&mut mock_buf, &mut buf).expect("decode");
        assert_eq!(16, count);
        let result = String::from_utf8(buf.to_vec()).expect("decode String");
        assert_eq!("1234567890abcdef", &result);
    }

    #[test]
    fn test_read_chunked_after_eof() {
        let content = b"10\r\n1234567890abcdef\r\n0\r\n\r\n";
        let mut mock_buf = io::Cursor::new(content);
        let mut buf = [0u8; 50];
        let mut decoder = Decoder::chunked();

        // normal read
        let count = decoder.decode(&mut mock_buf, &mut buf).expect("decode");
        assert_eq!(16, count);
        let result = String::from_utf8(buf[0..count].to_vec()).expect("decode String");
        assert_eq!("1234567890abcdef", &result);

        // eof read
        let count = decoder.decode(&mut mock_buf, &mut buf).expect("decode");
        assert_eq!(0, count);

        // ensure read after eof also returns eof
        let count = decoder.decode(&mut mock_buf, &mut buf).expect("decode");
        assert_eq!(0, count);
    }

    // perform an async read using a custom buffer size and causing a blocking
    // read at the specified byte
    fn read_async(mut decoder: Decoder,
                  content: &[u8],
                  block_at: usize,
                  read_buffer_size: usize)
                  -> String {
        let content_len = content.len();
        let mock_buf = io::Cursor::new(content.clone());
        let mut ins = Async::new(mock_buf, block_at);
        let mut outs = vec![];
        loop {
            let mut buf = vec![0; read_buffer_size];
            match decoder.decode(&mut ins, buf.as_mut_slice()) {
                Ok(0) => break,
                Ok(i) => outs.write(&buf[0..i]).expect("write buffer"),
                Err(e) => {
                    if e.kind() != io::ErrorKind::WouldBlock {
                        break;
                    }
                    ins.block_in(content_len); // we only block once
                    0 as usize
                }
            };
        }
        String::from_utf8(outs).expect("decode String")
    }

    // iterate over the different ways that this async read could go.
    // tests every combination of buffer size that is passed in, with a blocking
    // read at each byte along the content - The shotgun approach
    fn all_async_cases(content: &str, expected: &str, decoder: Decoder) {
        let content_len = content.len();
        for block_at in 0..content_len {
            for read_buffer_size in 1..content_len {
                let actual = read_async(decoder.clone(),
                                        content.as_bytes(),
                                        block_at,
                                        read_buffer_size);
                assert_eq!(expected,
                    &actual,
                    "Failed async. Blocking at {} with read buffer size {}",
                    block_at,
                    read_buffer_size);
            }
        }
    }

    #[test]
    fn test_read_length_async() {
        let content = "foobar";
        all_async_cases(content, content, Decoder::length(content.len() as u64));
    }

    #[test]
    fn test_read_chunked_async() {
        let content = "3\r\nfoo\r\n3\r\nbar\r\n0\r\n";
        let expected = "foobar";
        all_async_cases(content, expected, Decoder::chunked());
    }

    #[test]
    fn test_read_eof_async() {
        let content = "foobar";
        all_async_cases(content, content, Decoder::eof());
    }

}
