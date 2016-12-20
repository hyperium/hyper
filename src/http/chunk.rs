use std::fmt;

/// A piece of a message body.
pub struct Chunk(Inner);

enum Inner {
    Owned(Vec<u8>),
    Static(&'static [u8]),
}

impl From<Vec<u8>> for Chunk {
    #[inline]
    fn from(v: Vec<u8>) -> Chunk {
        Chunk(Inner::Owned(v))
    }
}

impl From<&'static [u8]> for Chunk {
    #[inline]
    fn from(slice: &'static [u8]) -> Chunk {
        Chunk(Inner::Static(slice))
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
            Inner::Owned(ref vec) => vec,
            Inner::Static(slice) => slice,
        }
    }
}

impl fmt::Debug for Chunk {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self.as_ref(), f)
    }
}
