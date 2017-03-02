use std::str;

use bytes::Bytes;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ByteStr(Bytes);

impl ByteStr {
    pub unsafe fn from_utf8_unchecked(slice: Bytes) -> ByteStr {
        ByteStr(slice)
    }

    pub fn as_str(&self) -> &str {
        unsafe { str::from_utf8_unchecked(self.0.as_ref()) }
    }
}
