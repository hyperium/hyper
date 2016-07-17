use std::borrow::Cow;
use std::fmt;

/// A raw header value.
#[derive(Clone, PartialEq, Eq)]
pub struct Raw(Lines);

impl Raw {
    /// Returns the amount of lines.
    #[inline]
    pub fn len(&self) -> usize {
        match self.0 {
            Lines::One(..) => 1,
            Lines::Many(ref lines) => lines.len()
        }
    }

    /// Returns the line if there is only 1.
    #[inline]
    pub fn one(&self) -> Option<&[u8]> {
        match self.0 {
            Lines::One(ref line) => Some(line),
            Lines::Many(ref lines) if lines.len() == 1 => Some(&lines[0]),
            _ => None
        }
    }

    /// Iterate the lines of raw bytes.
    #[inline]
    pub fn iter(&self) -> RawLines {
        RawLines {
            inner: match self.0 {
                Lines::One(ref line) => unsafe {
                    ::std::slice::from_raw_parts(line, 1)
                }.iter(),
                Lines::Many(ref lines) => lines.iter()
            }
        }
    }

    /// Append a line to this `Raw` header value.
    pub fn push(&mut self, val: &[u8]) {
        let lines = ::std::mem::replace(&mut self.0, Lines::Many(Vec::new()));
        match lines {
            Lines::One(line) => {
                self.0 = Lines::Many(vec![line, maybe_literal(val.into())]);
            }
            Lines::Many(mut lines) => {
                lines.push(maybe_literal(val.into()));
                self.0 = Lines::Many(lines);
            }
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
enum Lines {
    One(Line),
    Many(Vec<Line>)
}

type Line = Cow<'static, [u8]>;

fn eq<A: AsRef<[u8]>, B: AsRef<[u8]>>(a: &[A], b: &[B]) -> bool {
    if a.len() != b.len() {
        false
    } else {
        for (a, b) in a.iter().zip(b.iter()) {
            if a.as_ref() != b.as_ref() {
                return false
            }
        }
        true
    }
}

impl PartialEq<[Vec<u8>]> for Raw {
    fn eq(&self, bytes: &[Vec<u8>]) -> bool {
        match self.0 {
            Lines::One(ref line) => eq(&[line], bytes),
            Lines::Many(ref lines) => eq(lines, bytes)
        }
    }
}

impl PartialEq<[u8]> for Raw {
    fn eq(&self, bytes: &[u8]) -> bool {
        match self.0 {
            Lines::One(ref line) => line.as_ref() == bytes,
            Lines::Many(..) => false
        }
    }
}

impl PartialEq<str> for Raw {
    fn eq(&self, s: &str) -> bool {
        match self.0 {
            Lines::One(ref line) => line.as_ref() == s.as_bytes(),
            Lines::Many(..) => false
        }
    }
}

impl From<Vec<Vec<u8>>> for Raw {
    #[inline]
    fn from(val: Vec<Vec<u8>>) -> Raw {
        Raw(Lines::Many(
            val.into_iter()
                .map(|vec| maybe_literal(vec.into()))
                .collect()
        ))
    }
}

impl From<String> for Raw {
    #[inline]
    fn from(val: String) -> Raw {
        let vec: Vec<u8> = val.into();
        vec.into()
    }
}

impl From<Vec<u8>> for Raw {
    #[inline]
    fn from(val: Vec<u8>) -> Raw {
        Raw(Lines::One(Cow::Owned(val)))
    }
}

impl From<&'static str> for Raw {
    fn from(val: &'static str) -> Raw {
        Raw(Lines::One(Cow::Borrowed(val.as_bytes())))
    }
}

impl From<&'static [u8]> for Raw {
    fn from(val: &'static [u8]) -> Raw {
        Raw(Lines::One(Cow::Borrowed(val)))
    }
}

pub fn parsed(val: &[u8]) -> Raw {
    Raw(Lines::One(maybe_literal(val.into())))
}

impl fmt::Debug for Raw {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.0 {
            Lines::One(ref line) => fmt::Debug::fmt(&[line], f),
            Lines::Many(ref lines) => fmt::Debug::fmt(lines, f)
        }
    }
}

impl ::std::ops::Index<usize> for Raw {
    type Output = [u8];
    fn index(&self, idx: usize) -> &[u8] {
        match self.0 {
            Lines::One(ref line) => if idx == 0 {
                line.as_ref()
            } else {
                panic!("index out of bounds: {}", idx)
            },
            Lines::Many(ref lines) => lines[idx].as_ref()
        }
    }
}

macro_rules! literals {
    ($($len:expr => $($value:expr),+;)+) => (
        fn maybe_literal<'a>(s: Cow<'a, [u8]>) -> Cow<'static, [u8]> {
            match s.len() {
                $($len => {
                    $(
                    if s.as_ref() == $value {
                        return Cow::Borrowed($value);
                    }
                    )+
                })+

                _ => ()
            }

            Cow::Owned(s.into_owned())
        }

        #[test]
        fn test_literal_lens() {
            $(
            $({
                let s = $value;
                assert!(s.len() == $len, "{:?} has len of {}, listed as {}", s, s.len(), $len);
            })+
            )+
        }
    );
}

literals! {
    1  => b"*", b"0";
    3  => b"*/*";
    4  => b"gzip";
    5  => b"close";
    7  => b"chunked";
    10 => b"keep-alive";
}

impl<'a> IntoIterator for &'a Raw {
    type IntoIter = RawLines<'a>;
    type Item = &'a [u8];

    fn into_iter(self) -> RawLines<'a> {
        self.iter()
    }
}

#[derive(Debug)]
pub struct RawLines<'a> {
    inner: ::std::slice::Iter<'a, Cow<'static, [u8]>>
}

impl<'a> Iterator for RawLines<'a> {
    type Item = &'a [u8];

    #[inline]
    fn next(&mut self) -> Option<&'a [u8]> {
        self.inner.next().map(AsRef::as_ref)
    }
}
