use std::ops::Deref;
use std::str;

use bytes::Bytes;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ByteStr(Bytes);

impl ByteStr {
    pub unsafe fn from_utf8_unchecked(slice: Bytes) -> ByteStr {
        ByteStr(slice)
    }

    pub fn from_static(s: &'static str) -> ByteStr {
        ByteStr(Bytes::from_static(s.as_bytes()))
    }

    pub fn slice(&self, from: usize, to: usize) -> ByteStr {
        assert!(self.as_str().is_char_boundary(from));
        assert!(self.as_str().is_char_boundary(to));
        ByteStr(self.0.slice(from, to))
    }

    pub fn slice_to(&self, idx: usize) -> ByteStr {
        assert!(self.as_str().is_char_boundary(idx));
        ByteStr(self.0.slice_to(idx))
    }

    pub fn as_str(&self) -> &str {
        unsafe { str::from_utf8_unchecked(self.0.as_ref()) }
    }

    pub fn insert(&mut self, idx: usize, ch: char) {
        let mut s = self.as_str().to_owned();
        s.insert(idx, ch);
        let bytes = Bytes::from(s);
        self.0 = bytes;
    }

    #[cfg(feature = "compat")]
    pub fn into_bytes(self) -> Bytes {
        self.0
    }
}

impl Deref for ByteStr {
    type Target = str;
    fn deref(&self) -> &str {
        self.as_str()
    }
}

impl<'a> From<&'a str> for ByteStr {
    fn from(s: &'a str) -> ByteStr {
        ByteStr(Bytes::from(s))
    }
}
