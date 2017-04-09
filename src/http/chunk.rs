use std::fmt;
use std::mem;

use bytes::{Bytes, BytesMut, BufMut};

/// A piece of a message body.
pub struct Chunk(Inner);

enum Inner {
    Mut(BytesMut),
    Shared(Bytes),
    Swapping,
}

impl Inner {
    fn as_bytes_mut(&mut self, reserve: usize) -> &mut BytesMut {
        match *self {
            Inner::Mut(ref mut bytes) => return bytes,
            _ => ()
        }

        let bytes = match mem::replace(self, Inner::Swapping) {
            Inner::Shared(bytes) => bytes,
            _ => unreachable!(),
        };

        let bytes_mut = bytes.try_mut().unwrap_or_else(|bytes| {
            let mut bytes_mut = BytesMut::with_capacity(reserve + bytes.len());
            bytes_mut.put_slice(bytes.as_ref());
            bytes_mut
        });

        *self = Inner::Mut(bytes_mut);
        match *self {
            Inner::Mut(ref mut bytes) => bytes,
            _ => unreachable!(),
        }
    }
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
    fn from(mem: Bytes) -> Chunk {
        Chunk(Inner::Shared(mem))
    }
}

impl From<Chunk> for Bytes {
    fn from(chunk: Chunk) -> Bytes {
        match chunk.0 {
            Inner::Mut(bytes_mut) => bytes_mut.freeze(),
            Inner::Shared(bytes) => bytes,
            Inner::Swapping => unreachable!(),
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
            Inner::Mut(ref slice) => slice,
            Inner::Shared(ref slice) => slice,
            Inner::Swapping => unreachable!(),
        }
    }
}

impl fmt::Debug for Chunk {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self.as_ref(), f)
    }
}

impl IntoIterator for Chunk {
    type Item = u8;
    type IntoIter = <Bytes as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        match self.0 {
            Inner::Mut(bytes) => bytes.freeze().into_iter(),
            Inner::Shared(bytes) => bytes.into_iter(),
            Inner::Swapping => unreachable!(),
        }
    }
}

impl Extend<u8> for Chunk {
    fn extend<T>(&mut self, iter: T) where T: IntoIterator<Item=u8> {
        let iter = iter.into_iter();

        self.0.as_bytes_mut(iter.size_hint().0).extend(iter);
    }
}
