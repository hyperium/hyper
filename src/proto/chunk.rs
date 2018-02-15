use std::fmt;

use bytes::Bytes;

/// A piece of a message body.
pub struct Chunk(Inner);

enum Inner {
    Shared(Bytes),
}

impl From<Vec<u8>> for Chunk {
    #[inline]
    fn from(v: Vec<u8>) -> Chunk {
        Chunk::from(Bytes::from(v))
    }
}

impl From<&'static [u8]> for Chunk {
    #[inline]
    fn from(slice: &'static [u8]) -> Chunk {
        Chunk::from(Bytes::from_static(slice))
    }
}

impl From<String> for Chunk {
    #[inline]
    fn from(s: String) -> Chunk {
        s.into_bytes().into()
    }
}

impl From<&'static str> for Chunk {
    #[inline]
    fn from(slice: &'static str) -> Chunk {
        slice.as_bytes().into()
    }
}

impl From<Bytes> for Chunk {
    #[inline]
    fn from(mem: Bytes) -> Chunk {
        Chunk(Inner::Shared(mem))
    }
}

impl From<Chunk> for Bytes {
    #[inline]
    fn from(chunk: Chunk) -> Bytes {
        match chunk.0 {
            Inner::Shared(bytes) => bytes,
        }
    }
}

impl ::std::ops::Deref for Chunk {
    type Target = [u8];

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}

impl AsRef<[u8]> for Chunk {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        match self.0 {
            Inner::Shared(ref slice) => slice,
        }
    }
}

impl fmt::Debug for Chunk {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self.as_ref(), f)
    }
}

impl Default for Chunk {
    #[inline]
    fn default() -> Chunk {
        Chunk(Inner::Shared(Bytes::new()))
    }
}

impl IntoIterator for Chunk {
    type Item = u8;
    type IntoIter = <Bytes as IntoIterator>::IntoIter;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        match self.0 {
            Inner::Shared(bytes) => bytes.into_iter(),
        }
    }
}

impl Extend<u8> for Chunk {
    #[inline]
    fn extend<T>(&mut self, iter: T) where T: IntoIterator<Item=u8> {
        match self.0 {
            Inner::Shared(ref mut bytes) => bytes.extend(iter)
        }
    }
}
