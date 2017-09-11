use std::ops::Deref;
use std::str;

use bytes::Bytes;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ByteStr(Bytes);

impl ByteStr {
    pub fn as_str(&self) -> &str {
        unsafe { str::from_utf8_unchecked(self.0.as_ref()) }
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
