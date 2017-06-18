use std::cmp;
use std::io::{self, Write};

use bytes::BytesMut;

/// Encoders to handle different Transfer-Encodings.
#[derive(Debug, Clone)]
pub struct Encoder {
    kind: Kind,
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
        }
    }

    pub fn length(len: u64) -> Encoder {
        Encoder {
            kind: Kind::Length(len),
        }
    }

    pub fn is_eof(&self) -> bool {
        match self.kind {
            Kind::Length(0) |
            Kind::Chunked(Chunked::End) => true,
            _ => false
        }
    }

    pub fn eof(&self) -> Result<Option<&'static [u8]>, NotEof> {
        match self.kind {
            Kind::Length(0) => Ok(None),
            Kind::Chunked(Chunked::Init) => Ok(Some(b"0\r\n\r\n")),
            _ => Err(NotEof),
        }
    }

    pub fn encode<W: Write>(&mut self, w: &mut W, msg: &[u8], result_buf: &mut BytesMut) -> io::Result<usize> {
        match self.kind {
            Kind::Chunked(ref mut chunked) => {
                chunked.encode(w, msg, result_buf)
            },
            Kind::Length(ref mut remaining) => {
                let n = {
                    let max = cmp::min(*remaining as usize, msg.len());
                    trace!("sized write, len = {}", max);
                    let slice = &msg[..max];

                    // TODO: Could we push this out of encode?
                    try!(w.write(slice))
                };

                if n == 0 {
                    return Err(io::Error::new(io::ErrorKind::WouldBlock, "would block"));
                }

                *remaining -= n as u64;
                debug!("encoded {} bytes", n);
                trace!("encode sized complete, remaining = {}", remaining);
                Ok(n)
            },
        }
    }
}

#[derive(Debug)]
pub struct NotEof;

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
    fn encode<W: Write>(&mut self, w: &mut W, msg: &[u8], result_buf: &mut BytesMut) -> io::Result<usize> {
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
            // TODO: Find a better way to concatente into result_buf
            match *self {
                Chunked::Init => unreachable!("Chunked::Init should have become Chunked::Size"),
                Chunked::Size(ref size) => {
                    result_buf.extend_from_slice(&size.bytes[size.pos.into() .. size.len.into()]);
                    result_buf.extend_from_slice(&b"\r\n"[..]);
                    result_buf.extend_from_slice(msg);
                    result_buf.extend_from_slice(&b"\r\n"[..]);
                },
                Chunked::SizeCr => {
                    result_buf.extend_from_slice(&b"\r\n"[..]);
                    result_buf.extend_from_slice(msg);
                    result_buf.extend_from_slice(&b"\r\n"[..]);
                },
                Chunked::SizeLf => {
                    result_buf.extend_from_slice(&b"\n"[..]);
                    result_buf.extend_from_slice(msg);
                    result_buf.extend_from_slice(&b"\r\n"[..]);
                },
                Chunked::Body(pos) => {
                    result_buf.extend_from_slice(&msg[pos..]);
                    result_buf.extend_from_slice(&b"\r\n"[..]);
                },
                Chunked::BodyCr => {
                    result_buf.extend_from_slice(&b"\r\n"[..]);
                },
                Chunked::BodyLf => {
                    result_buf.extend_from_slice(&b"\n"[..]);
                },
                Chunked::End => unreachable!("Chunked::End shouldn't write more")
            };
            // TODO: Could we push this out of encode?
            try!(w.write(&result_buf.take()))
        };
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

#[cfg(test)]
mod tests {
    use super::Encoder;
    use mock::{AsyncIo, Buf};
    use bytes::{BytesMut};

    #[test]
    fn test_chunked_encode_sync() {
        let mut dst = Buf::new();
        let mut encoder = Encoder::chunked();
        let mut result_buf = BytesMut::with_capacity(0);

        encoder.encode(&mut dst, b"foo bar", &mut result_buf).unwrap();
        encoder.encode(&mut dst, b"baz quux herp", &mut result_buf).unwrap();
        encoder.encode(&mut dst, b"", &mut result_buf).unwrap();
        assert_eq!(&dst[..], &b"7\r\nfoo bar\r\nD\r\nbaz quux herp\r\n0\r\n\r\n"[..]);
    }

    #[test]
    fn test_chunked_encode_async() {
        let mut dst = AsyncIo::new(Buf::new(), 7);
        let mut encoder = Encoder::chunked();
        let mut result_buf = BytesMut::with_capacity(0);

        assert!(encoder.encode(&mut dst, b"foo bar", &mut result_buf).is_err());
        dst.block_in(6);
        assert_eq!(7, encoder.encode(&mut dst, b"foo bar", &mut result_buf).unwrap());
        dst.block_in(30);
        assert_eq!(13, encoder.encode(&mut dst, b"baz quux herp", &mut result_buf).unwrap());
        encoder.encode(&mut dst, b"", &mut result_buf).unwrap();
        assert_eq!(&dst[..], &b"7\r\nfoo bar\r\nD\r\nbaz quux herp\r\n0\r\n\r\n"[..]);
    }

    #[test]
    fn test_sized_encode() {
        let mut dst = Buf::new();
        let mut encoder = Encoder::length(8);
        let mut result_buf = BytesMut::with_capacity(0);

        encoder.encode(&mut dst, b"foo bar", &mut result_buf).unwrap();
        assert_eq!(encoder.encode(&mut dst, b"baz", &mut result_buf).unwrap(), 1);

        assert_eq!(dst, b"foo barb");
    }
}
