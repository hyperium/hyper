use std::fmt;

use bytes::Bytes;
use h2::ReleaseCapacity;

/// A piece of a message body.
///
/// These are returned by [`Body`](::Body). It is an efficient buffer type,
/// and wraps auto-management of flow control in the case of HTTP2 messages.
///
/// A `Chunk` can be easily created by many of Rust's standard types that
/// represent a collection of bytes, using `Chunk::from`.
pub struct Chunk {
    /// The buffer of bytes making up this body.
    bytes: Bytes,
    /// A possible HTTP2 marker to ensure we release window capacity.
    ///
    /// This version just automatically releases all capacity when `Chunk`
    /// is dropped.
    _flow_control: Option<AutoRelease>,
}

struct AutoRelease {
    cap: usize,
    release: ReleaseCapacity,
}

impl Drop for AutoRelease {
    fn drop(&mut self) {
        let _ = self.release.release_capacity(self.cap);
    }
}

impl Chunk {
    pub(crate) fn h2(bytes: Bytes, rel_cap: &ReleaseCapacity) -> Chunk {
        let cap = bytes.len();
        Chunk {
            bytes: bytes,
            _flow_control: Some(AutoRelease {
                cap: cap,
                release: rel_cap.clone(),
            }),
        }
    }

    /// Converts this `Chunk` directly into the `Bytes` type without copies.
    ///
    /// This is simply an inherent alias for `Bytes::from(chunk)`, which exists,
    /// but doesn't appear in rustdocs.
    #[inline]
    pub fn into_bytes(self) -> Bytes {
        self.into()
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
    #[inline]
    fn from(bytes: Bytes) -> Chunk {
        Chunk {
            bytes: bytes,
            _flow_control: None,
        }
    }
}

impl From<Chunk> for Bytes {
    #[inline]
    fn from(chunk: Chunk) -> Bytes {
        chunk.bytes
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
        &self.bytes
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
        Chunk::from(Bytes::new())
    }
}

impl IntoIterator for Chunk {
    type Item = u8;
    type IntoIter = <Bytes as IntoIterator>::IntoIter;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.bytes.into_iter()
    }
}

impl Extend<u8> for Chunk {
    #[inline]
    fn extend<T>(&mut self, iter: T) where T: IntoIterator<Item=u8> {
        self.bytes.extend(iter)
    }
}
