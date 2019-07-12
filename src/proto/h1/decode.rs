use std::error::Error as StdError;
use std::fmt;
use std::usize;
use std::io;

use bytes::Bytes;

use crate::common::{Poll, task};

use super::io::MemRead;
use super::{DecodedLength};

use self::Kind::{Length, Chunked, Eof};

/// Decoders to handle different Transfer-Encodings.
///
/// If a message body does not include a Transfer-Encoding, it *should*
/// include a Content-Length header.
#[derive(Clone, PartialEq)]
pub struct Decoder {
    kind: Kind,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Kind {
    /// A Reader used when a Content-Length header is passed with a positive integer.
    Length(u64),
    /// A Reader used when Transfer-Encoding is `chunked`.
    Chunked(ChunkedState, u64),
    /// A Reader used for responses that don't indicate a length or chunked.
    ///
    /// The bool tracks when EOF is seen on the transport.
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

#[derive(Debug, PartialEq, Clone, Copy)]
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
    // constructors

    pub fn length(x: u64) -> Decoder {
        Decoder { kind: Kind::Length(x) }
    }

    pub fn chunked() -> Decoder {
        Decoder { kind: Kind::Chunked(ChunkedState::Size, 0) }
    }

    pub fn eof() -> Decoder {
        Decoder { kind: Kind::Eof(false) }
    }

    pub(super) fn new(len: DecodedLength) -> Self {
        match len {
            DecodedLength::CHUNKED => Decoder::chunked(),
            DecodedLength::CLOSE_DELIMITED => Decoder::eof(),
            length => Decoder::length(length.danger_len()),
        }
    }

    // methods

    pub fn is_eof(&self) -> bool {
        match self.kind {
            Length(0) |
            Chunked(ChunkedState::End, _) |
            Eof(true) => true,
            _ => false,
        }
    }

    pub fn decode<R: MemRead>(&mut self, cx: &mut task::Context<'_>, body: &mut R) -> Poll<Result<Bytes, io::Error>> {
        trace!("decode; state={:?}", self.kind);
        match self.kind {
            Length(ref mut remaining) => {
                if *remaining == 0 {
                    Poll::Ready(Ok(Bytes::new()))
                } else {
                    let to_read = *remaining as usize;
                    let buf = ready!(body.read_mem(cx, to_read))?;
                    let num = buf.as_ref().len() as u64;
                    if num > *remaining {
                        *remaining = 0;
                    } else if num == 0 {
                        return Poll::Ready(Err(io::Error::new(io::ErrorKind::UnexpectedEof, IncompleteBody)));
                    } else {
                        *remaining -= num;
                    }
                    Poll::Ready(Ok(buf))
                }
            }
            Chunked(ref mut state, ref mut size) => {
                loop {
                    let mut buf = None;
                    // advances the chunked state
                    *state = ready!(state.step(cx, body, size, &mut buf))?;
                    if *state == ChunkedState::End {
                        trace!("end of chunked");
                        return Poll::Ready(Ok(Bytes::new()));
                    }
                    if let Some(buf) = buf {
                        return Poll::Ready(Ok(buf));
                    }
                }
            }
            Eof(ref mut is_eof) => {
                if *is_eof {
                    Poll::Ready(Ok(Bytes::new()))
                } else {
                    // 8192 chosen because its about 2 packets, there probably
                    // won't be that much available, so don't have MemReaders
                    // allocate buffers to big
                    body.read_mem(cx, 8192).map_ok(|slice| {
                        *is_eof = slice.is_empty();
                        slice
                    })
                }
            }
        }
    }
}


impl fmt::Debug for Decoder {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&self.kind, f)
    }
}

macro_rules! byte (
    ($rdr:ident, $cx:expr) => ({
        let buf = ready!($rdr.read_mem($cx, 1))?;
        if !buf.is_empty() {
            buf[0]
        } else {
            return Poll::Ready(Err(io::Error::new(io::ErrorKind::UnexpectedEof,
                                      "unexpected EOF during chunk size line")));
        }
    })
);

impl ChunkedState {
    fn step<R: MemRead>(&self,
                        cx: &mut task::Context<'_>,
                        body: &mut R,
                        size: &mut u64,
                        buf: &mut Option<Bytes>)
                        -> Poll<Result<ChunkedState, io::Error>> {
        use self::ChunkedState::*;
        match *self {
            Size => ChunkedState::read_size(cx, body, size),
            SizeLws => ChunkedState::read_size_lws(cx, body),
            Extension => ChunkedState::read_extension(cx, body),
            SizeLf => ChunkedState::read_size_lf(cx, body, *size),
            Body => ChunkedState::read_body(cx, body, size, buf),
            BodyCr => ChunkedState::read_body_cr(cx, body),
            BodyLf => ChunkedState::read_body_lf(cx, body),
            EndCr => ChunkedState::read_end_cr(cx, body),
            EndLf => ChunkedState::read_end_lf(cx, body),
            End => Poll::Ready(Ok(ChunkedState::End)),
        }
    }
    fn read_size<R: MemRead>(cx: &mut task::Context<'_>, rdr: &mut R, size: &mut u64) -> Poll<Result<ChunkedState, io::Error>> {
        trace!("Read chunk hex size");
        let radix = 16;
        match byte!(rdr, cx) {
            b @ b'0'..=b'9' => {
                *size *= radix;
                *size += (b - b'0') as u64;
            }
            b @ b'a'..=b'f' => {
                *size *= radix;
                *size += (b + 10 - b'a') as u64;
            }
            b @ b'A'..=b'F' => {
                *size *= radix;
                *size += (b + 10 - b'A') as u64;
            }
            b'\t' | b' ' => return Poll::Ready(Ok(ChunkedState::SizeLws)),
            b';' => return Poll::Ready(Ok(ChunkedState::Extension)),
            b'\r' => return Poll::Ready(Ok(ChunkedState::SizeLf)),
            _ => {
                return Poll::Ready(Err(io::Error::new(io::ErrorKind::InvalidInput,
                                          "Invalid chunk size line: Invalid Size")));
            }
        }
        Poll::Ready(Ok(ChunkedState::Size))
    }
    fn read_size_lws<R: MemRead>(cx: &mut task::Context<'_>, rdr: &mut R) -> Poll<Result<ChunkedState, io::Error>> {
        trace!("read_size_lws");
        match byte!(rdr, cx) {
            // LWS can follow the chunk size, but no more digits can come
            b'\t' | b' ' => Poll::Ready(Ok(ChunkedState::SizeLws)),
            b';' => Poll::Ready(Ok(ChunkedState::Extension)),
            b'\r' => Poll::Ready(Ok(ChunkedState::SizeLf)),
            _ => {
                Poll::Ready(Err(io::Error::new(io::ErrorKind::InvalidInput,
                                   "Invalid chunk size linear white space")))
            }
        }
    }
    fn read_extension<R: MemRead>(cx: &mut task::Context<'_>, rdr: &mut R) -> Poll<Result<ChunkedState, io::Error>> {
        trace!("read_extension");
        match byte!(rdr, cx) {
            b'\r' => Poll::Ready(Ok(ChunkedState::SizeLf)),
            _ => Poll::Ready(Ok(ChunkedState::Extension)), // no supported extensions
        }
    }
    fn read_size_lf<R: MemRead>(cx: &mut task::Context<'_>, rdr: &mut R, size: u64) -> Poll<Result<ChunkedState, io::Error>> {
        trace!("Chunk size is {:?}", size);
        match byte!(rdr, cx) {
            b'\n' => {
                if size == 0 {
                    Poll::Ready(Ok(ChunkedState::EndCr))
                } else {
                    debug!("incoming chunked header: {0:#X} ({0} bytes)", size);
                    Poll::Ready(Ok(ChunkedState::Body))
                }
            },
            _ => Poll::Ready(Err(io::Error::new(io::ErrorKind::InvalidInput, "Invalid chunk size LF"))),
        }
    }

    fn read_body<R: MemRead>(cx: &mut task::Context<'_>, rdr: &mut R,
                          rem: &mut u64,
                          buf: &mut Option<Bytes>)
                          -> Poll<Result<ChunkedState, io::Error>> {
        trace!("Chunked read, remaining={:?}", rem);

        // cap remaining bytes at the max capacity of usize
        let rem_cap = match *rem {
            r if r > usize::MAX as u64 => usize::MAX,
            r => r as usize,
        };

        let to_read = rem_cap;
        let slice = ready!(rdr.read_mem(cx, to_read))?;
        let count = slice.len();

        if count == 0 {
            *rem = 0;
            return Poll::Ready(Err(io::Error::new(io::ErrorKind::UnexpectedEof, IncompleteBody)));
        }
        *buf = Some(slice);
        *rem -= count as u64;

        if *rem > 0 {
            Poll::Ready(Ok(ChunkedState::Body))
        } else {
            Poll::Ready(Ok(ChunkedState::BodyCr))
        }
    }
    fn read_body_cr<R: MemRead>(cx: &mut task::Context<'_>, rdr: &mut R) -> Poll<Result<ChunkedState, io::Error>> {
        match byte!(rdr, cx) {
            b'\r' => Poll::Ready(Ok(ChunkedState::BodyLf)),
            _ => Poll::Ready(Err(io::Error::new(io::ErrorKind::InvalidInput, "Invalid chunk body CR"))),
        }
    }
    fn read_body_lf<R: MemRead>(cx: &mut task::Context<'_>, rdr: &mut R) -> Poll<Result<ChunkedState, io::Error>> {
        match byte!(rdr, cx) {
            b'\n' => Poll::Ready(Ok(ChunkedState::Size)),
            _ => Poll::Ready(Err(io::Error::new(io::ErrorKind::InvalidInput, "Invalid chunk body LF"))),
        }
    }

    fn read_end_cr<R: MemRead>(cx: &mut task::Context<'_>, rdr: &mut R) -> Poll<Result<ChunkedState, io::Error>> {
        match byte!(rdr, cx) {
            b'\r' => Poll::Ready(Ok(ChunkedState::EndLf)),
            _ => Poll::Ready(Err(io::Error::new(io::ErrorKind::InvalidInput, "Invalid chunk end CR"))),
        }
    }
    fn read_end_lf<R: MemRead>(cx: &mut task::Context<'_>, rdr: &mut R) -> Poll<Result<ChunkedState, io::Error>> {
        match byte!(rdr, cx) {
            b'\n' => Poll::Ready(Ok(ChunkedState::End)),
            _ => Poll::Ready(Err(io::Error::new(io::ErrorKind::InvalidInput, "Invalid chunk end LF"))),
        }
    }
}

#[derive(Debug)]
struct IncompleteBody;

impl fmt::Display for IncompleteBody {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.description())
    }
}

impl StdError for IncompleteBody {
    fn description(&self) -> &str {
        "end of file before message length reached"
    }
}

#[cfg(test)]
mod tests {
    // FIXME: re-implement tests with `async/await`, this import should
    // trigger a warning to remind us
    use crate::Error;
    /*
    use std::io;
    use std::io::Write;
    use super::Decoder;
    use super::ChunkedState;
    use super::super::io::MemRead;
    use futures::{Async, Poll};
    use bytes::{BytesMut, Bytes};
    use crate::mock::AsyncIo;

    impl<'a> MemRead for &'a [u8] {
        fn read_mem(&mut self, len: usize) -> Poll<Result<Bytes, io::Error>> {
            let n = ::std::cmp::min(len, self.len());
            if n > 0 {
                let (a, b) = self.split_at(n);
                let mut buf = BytesMut::from(a);
                *self = b;
                Poll::Ready(Ok(buf.split_to(n).freeze()))
            } else {
                Poll::Ready(Ok(Bytes::new()))
            }
        }
    }

    trait HelpUnwrap<T> {
        fn unwrap(self) -> T;
    }
    impl HelpUnwrap<Bytes> for Async<Bytes> {
        fn unwrap(self) -> Bytes {
            match self {
                Async::Ready(bytes) => bytes,
                Async::NotReady => panic!(),
            }
        }
    }
    impl HelpUnwrap<ChunkedState> for Async<ChunkedState> {
        fn unwrap(self) -> ChunkedState {
            match self {
                Async::Ready(state) => state,
                Async::NotReady => panic!(),
            }
        }
    }

    #[test]
    fn test_read_chunk_size() {
        use std::io::ErrorKind::{UnexpectedEof, InvalidInput};

        fn read(s: &str) -> u64 {
            let mut state = ChunkedState::Size;
            let rdr = &mut s.as_bytes();
            let mut size = 0;
            loop {
                let result = state.step(rdr, &mut size, &mut None);
                let desc = format!("read_size failed for {:?}", s);
                state = result.expect(desc.as_str()).unwrap();
                if state == ChunkedState::Body || state == ChunkedState::EndCr {
                    break;
                }
            }
            size
        }

        fn read_err(s: &str, expected_err: io::ErrorKind) {
            let mut state = ChunkedState::Size;
            let rdr = &mut s.as_bytes();
            let mut size = 0;
            loop {
                let result = state.step(rdr, &mut size, &mut None);
                state = match result {
                    Ok(s) => s.unwrap(),
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
        assert_eq!(decoder.decode(&mut bytes).unwrap().unwrap().len(), 7);
        let e = decoder.decode(&mut bytes).unwrap_err();
        assert_eq!(e.kind(), io::ErrorKind::UnexpectedEof);
    }

    #[test]
    fn test_read_chunked_early_eof() {
        let mut bytes = &b"\
            9\r\n\
            foo bar\
        "[..];
        let mut decoder = Decoder::chunked();
        assert_eq!(decoder.decode(&mut bytes).unwrap().unwrap().len(), 7);
        let e = decoder.decode(&mut bytes).unwrap_err();
        assert_eq!(e.kind(), io::ErrorKind::UnexpectedEof);
    }

    #[test]
    fn test_read_chunked_single_read() {
        let mut mock_buf = &b"10\r\n1234567890abcdef\r\n0\r\n"[..];
        let buf = Decoder::chunked().decode(&mut mock_buf).expect("decode").unwrap();
        assert_eq!(16, buf.len());
        let result = String::from_utf8(buf.as_ref().to_vec()).expect("decode String");
        assert_eq!("1234567890abcdef", &result);
    }

    #[test]
    fn test_read_chunked_after_eof() {
        let mut mock_buf = &b"10\r\n1234567890abcdef\r\n0\r\n\r\n"[..];
        let mut decoder = Decoder::chunked();

        // normal read
        let buf = decoder.decode(&mut mock_buf).expect("decode").unwrap();
        assert_eq!(16, buf.len());
        let result = String::from_utf8(buf.as_ref().to_vec()).expect("decode String");
        assert_eq!("1234567890abcdef", &result);

        // eof read
        let buf = decoder.decode(&mut mock_buf).expect("decode").unwrap();
        assert_eq!(0, buf.len());

        // ensure read after eof also returns eof
        let buf = decoder.decode(&mut mock_buf).expect("decode").unwrap();
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
            match decoder.decode(&mut ins).expect("unexpected decode error: {}") {
                Async::Ready(buf) => {
                    if buf.is_empty() {
                        break; // eof
                    }
                    outs.write(buf.as_ref()).expect("write buffer");
                },
                Async::NotReady => {
                    ins.block_in(content_len); // we only block once
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
            assert_eq!(expected, &actual) //, "Failed async. Blocking at {}", block_at);
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
    */
}
