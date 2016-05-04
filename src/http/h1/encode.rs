use std::borrow::Cow;
use std::cmp;
use std::io::{self, Write};

use http::internal::{AtomicWrite, WriteBuf};

/// Encoders to handle different Transfer-Encodings.
#[derive(Debug, Clone)]
pub struct Encoder {
    kind: Kind,
    prefix: Prefix, //Option<WriteBuf<Vec<u8>>>
}

#[derive(Debug, PartialEq, Clone)]
enum Kind {
    /// An Encoder for when Transfer-Encoding includes `chunked`.
    Chunked(Chunked),
    /// An Encoder for when Content-Length is set.
    ///
    /// Enforces that the body is not longer than the Content-Length header.
    Length(u64),
}

impl Encoder {
    pub fn chunked() -> Encoder {
        Encoder {
            kind: Kind::Chunked(Chunked::Init),
            prefix: Prefix(None)
        }
    }

    pub fn length(len: u64) -> Encoder {
        Encoder {
            kind: Kind::Length(len),
            prefix: Prefix(None)
        }
    }

    pub fn prefix(&mut self, prefix: WriteBuf<Vec<u8>>) {
        self.prefix.0 = Some(prefix);
    }

    pub fn is_eof(&self) -> bool {
        if self.prefix.0.is_some() {
            return false;
        }
        match self.kind {
            Kind::Length(0) |
            Kind::Chunked(Chunked::End) => true,
            _ => false
        }
    }

    pub fn end(self) -> Option<WriteBuf<Cow<'static, [u8]>>> {
        let trailer = self.trailer();
        let buf = self.prefix.0;

        match (buf, trailer) {
            (Some(mut buf), Some(trailer)) => {
                buf.bytes.extend_from_slice(trailer);
                Some(WriteBuf {
                    bytes: Cow::Owned(buf.bytes),
                    pos: buf.pos,
                })
            },
            (Some(buf), None) => Some(WriteBuf {
                bytes: Cow::Owned(buf.bytes),
                pos: buf.pos
            }),
            (None, Some(trailer)) => {
                Some(WriteBuf {
                    bytes: Cow::Borrowed(trailer),
                    pos: 0,
                })
            },
            (None, None) => None
        }
    }

    fn trailer(&self) -> Option<&'static [u8]> {
        match self.kind {
            Kind::Chunked(Chunked::Init) => {
                Some(b"0\r\n\r\n")
            }
            _ => None
        }
    }

    pub fn encode<W: AtomicWrite>(&mut self, w: &mut W, msg: &[u8]) -> io::Result<usize> {
        match self.kind {
            Kind::Chunked(ref mut chunked) => {
                chunked.encode(w, &mut self.prefix, msg)
            },
            Kind::Length(ref mut remaining) => {
                let mut n = {
                    let max = cmp::min(*remaining as usize, msg.len());
                    let slice = &msg[..max];

                    let prefix = self.prefix.0.as_ref().map(|buf| &buf.bytes[buf.pos..]).unwrap_or(b"");

                    try!(w.write_atomic(&[prefix, slice]))
                };

                n = self.prefix.update(n);
                if n == 0 {
                    return Err(io::Error::new(io::ErrorKind::WouldBlock, "would block"));
                }

                *remaining -= n as u64;
                Ok(n)
            },
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
enum Chunked {
    Init,
    Size(ChunkSize),
    SizeCr,
    SizeLf,
    Body(usize),
    BodyCr,
    BodyLf,
    End,
}

impl Chunked {
    fn encode<W: AtomicWrite>(&mut self, w: &mut W, prefix: &mut Prefix, msg: &[u8]) -> io::Result<usize> {
        match *self {
            Chunked::Init => {
                let mut size = ChunkSize {
                    bytes: [0; CHUNK_SIZE_MAX_BYTES],
                    pos: 0,
                    len: 0,
                };
                trace!("chunked write, size = {:?}", msg.len());
                write!(&mut size, "{:X}", msg.len())
                    .expect("CHUNK_SIZE_MAX_BYTES should fit any usize");
                *self = Chunked::Size(size);
            }
            Chunked::End => return Ok(0),
            _ => {}
        }
        let mut n = {
            let pieces = match *self {
                Chunked::Init => unreachable!("Chunked::Init should have become Chunked::Size"),
                Chunked::Size(ref size) => [
                    prefix.0.as_ref().map(|buf| &buf.bytes[buf.pos..]).unwrap_or(b""),
                    &size.bytes[size.pos.into() .. size.len.into()],
                    &b"\r\n"[..],
                    msg,
                    &b"\r\n"[..],
                ],
                Chunked::SizeCr => [
                    &b""[..],
                    &b""[..],
                    &b"\r\n"[..],
                    msg,
                    &b"\r\n"[..],
                ],
                Chunked::SizeLf => [
                    &b""[..],
                    &b""[..],
                    &b"\n"[..],
                    msg,
                    &b"\r\n"[..],
                ],
                Chunked::Body(pos) => [
                    &b""[..],
                    &b""[..],
                    &b""[..],
                    &msg[pos..],
                    &b"\r\n"[..],
                ],
                Chunked::BodyCr => [
                    &b""[..],
                    &b""[..],
                    &b""[..],
                    &b""[..],
                    &b"\r\n"[..],
                ],
                Chunked::BodyLf => [
                    &b""[..],
                    &b""[..],
                    &b""[..],
                    &b""[..],
                    &b"\n"[..],
                ],
                Chunked::End => unreachable!("Chunked::End shouldn't write more")
            };
            try!(w.write_atomic(&pieces))
        };

        if n > 0 {
            n = prefix.update(n);
        }
        while n > 0 {
            match *self {
                Chunked::Init => unreachable!("Chunked::Init should have become Chunked::Size"),
                Chunked::Size(mut size) => {
                    n = size.update(n);
                    if size.len == 0 {
                        *self = Chunked::SizeCr;
                    } else {
                        *self = Chunked::Size(size);
                    }
                },
                Chunked::SizeCr => {
                    *self = Chunked::SizeLf;
                    n -= 1;
                }
                Chunked::SizeLf => {
                    *self = Chunked::Body(0);
                    n -= 1;
                }
                Chunked::Body(pos) => {
                    let left = msg.len() - pos;
                    if n >= left {
                        *self = Chunked::BodyCr;
                        n -= left;
                    } else {
                        *self = Chunked::Body(pos + n);
                        n = 0;
                    }
                }
                Chunked::BodyCr => {
                    *self = Chunked::BodyLf;
                    n -= 1;
                }
                Chunked::BodyLf => {
                    assert!(n == 1);
                    *self = if msg.len() == 0 {
                        Chunked::End
                    } else {
                        Chunked::Init
                    };
                    n = 0;
                },
                Chunked::End => unreachable!("Chunked::End shouldn't have any to write")
            }
        }

        match *self {
            Chunked::Init |
            Chunked::End => Ok(msg.len()),
            _ => Err(io::Error::new(io::ErrorKind::WouldBlock, "chunked incomplete"))
        }
    }
}

#[cfg(target_pointer_width = "32")]
const USIZE_BYTES: usize = 4;

#[cfg(target_pointer_width = "64")]
const USIZE_BYTES: usize = 8;

// each byte will become 2 hex
const CHUNK_SIZE_MAX_BYTES: usize = USIZE_BYTES * 2;

#[derive(Clone, Copy)]
struct ChunkSize {
    bytes: [u8; CHUNK_SIZE_MAX_BYTES],
    pos: u8,
    len: u8,
}

impl ChunkSize {
    fn update(&mut self, n: usize) -> usize {
        let diff = (self.len - self.pos).into();
        if n >= diff {
            self.pos = 0;
            self.len = 0;
            n - diff
        } else {
            self.pos += n as u8; // just verified it was a small usize
            0
        }
    }
}

impl ::std::fmt::Debug for ChunkSize {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        f.debug_struct("ChunkSize")
            .field("bytes", &&self.bytes[..self.len.into()])
            .field("pos", &self.pos)
            .finish()
    }
}

impl ::std::cmp::PartialEq for ChunkSize {
    fn eq(&self, other: &ChunkSize) -> bool {
        self.len == other.len &&
            self.pos == other.pos &&
            (&self.bytes[..]) == (&other.bytes[..])
    }
}

impl io::Write for ChunkSize {
    fn write(&mut self, msg: &[u8]) -> io::Result<usize> {
        let n = (&mut self.bytes[self.len.into() ..]).write(msg)
            .expect("&mut [u8].write() cannot error");
        self.len += n as u8; // safe because bytes is never bigger than 256
        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct Prefix(Option<WriteBuf<Vec<u8>>>);

impl Prefix {
    fn update(&mut self, n: usize) -> usize {
        if let Some(mut buf) = self.0.take() {
            if buf.bytes.len() - buf.pos > n {
                buf.pos += n;
                self.0 = Some(buf);
                0
            } else {
                let nbuf = buf.bytes.len() - buf.pos;
                n - nbuf
            }
        } else {
            n
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Encoder;
    use mock::{Async, Buf};

    #[test]
    fn test_write_chunked_sync() {
        let mut dst = Buf::new();
        let mut encoder = Encoder::chunked();

        encoder.encode(&mut dst, b"foo bar").unwrap();
        encoder.encode(&mut dst, b"baz quux herp").unwrap();
        encoder.encode(&mut dst, b"").unwrap();
        assert_eq!(&dst[..], &b"7\r\nfoo bar\r\nD\r\nbaz quux herp\r\n0\r\n\r\n"[..]);
    }

    #[test]
    fn test_write_chunked_async() {
        let mut dst = Async::new(Buf::new(), 7);
        let mut encoder = Encoder::chunked();

        assert!(encoder.encode(&mut dst, b"foo bar").is_err());
        dst.block_in(6);
        assert_eq!(7, encoder.encode(&mut dst, b"foo bar").unwrap());
        dst.block_in(30);
        assert_eq!(13, encoder.encode(&mut dst, b"baz quux herp").unwrap());
        encoder.encode(&mut dst, b"").unwrap();
        assert_eq!(&dst[..], &b"7\r\nfoo bar\r\nD\r\nbaz quux herp\r\n0\r\n\r\n"[..]);
    }

    #[test]
    fn test_write_sized() {
        let mut dst = Buf::new();
        let mut encoder = Encoder::length(8);
        encoder.encode(&mut dst, b"foo bar").unwrap();
        assert_eq!(encoder.encode(&mut dst, b"baz").unwrap(), 1);

        assert_eq!(dst, b"foo barb");
    }
}
