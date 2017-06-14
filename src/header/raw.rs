use std::borrow::Cow;
use std::fmt;
use bytes::Bytes;

/// A raw header value.
#[derive(Clone, Debug)]
pub struct Raw(Lines);

impl Raw {
    /// Returns the amount of lines.
    #[inline]
    pub fn len(&self) -> usize {
        match self.0 {
            Lines::Empty => 0,
            Lines::One(..) => 1,
            Lines::Many(ref lines) => lines.len()
        }
    }

    /// Returns the line if there is only 1.
    #[inline]
    pub fn one(&self) -> Option<&[u8]> {
        match self.0 {
            Lines::One(ref line) => Some(line.as_ref()),
            Lines::Many(ref lines) if lines.len() == 1 => Some(lines[0].as_ref()),
            _ => None
        }
    }

    /// Iterate the lines of raw bytes.
    #[inline]
    pub fn iter(&self) -> RawLines {
        RawLines {
            inner: &self.0,
            pos: 0,
        }
    }

    /// Append a line to this `Raw` header value.
    pub fn push<V: Into<Raw>>(&mut self, val: V) {
        let raw = val.into();
        match raw.0 {
            Lines::Empty => (),
            Lines::One(one) => self.push_line(one),
            Lines::Many(lines) => {
                for line in lines {
                    self.push_line(line);
                }
            }
        }
    }

    fn push_line(&mut self, line: Bytes) {
        let lines = ::std::mem::replace(&mut self.0, Lines::Empty);
        match lines {
            Lines::Empty => {
                self.0 = Lines::One(line);
            }
            Lines::One(one) => {
                self.0 = Lines::Many(vec![one, line]);
            }
            Lines::Many(mut lines) => {
                lines.push(line);
                self.0 = Lines::Many(lines);
            }
        }
    }
}

#[derive(Clone)]
enum Lines {
    Empty,
    One(Bytes),
    Many(Vec<Bytes>),
}

fn eq_many<A: AsRef<[u8]>, B: AsRef<[u8]>>(a: &[A], b: &[B]) -> bool {
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

fn eq<B: AsRef<[u8]>>(raw: &Raw, b: &[B]) -> bool {
    match raw.0 {
        Lines::Empty => b.is_empty(),
        Lines::One(ref line) => eq_many(&[line], b),
        Lines::Many(ref lines) => eq_many(lines, b)
    }
}

impl PartialEq for Raw {
    fn eq(&self, other: &Raw) -> bool {
        match other.0 {
            Lines::Empty => eq(self, &[] as &[Bytes]),
            Lines::One(ref line) => eq(self, &[line]),
            Lines::Many(ref lines) => eq(self, lines),
        }
    }
}

impl Eq for Raw {}

impl PartialEq<[Vec<u8>]> for Raw {
    fn eq(&self, bytes: &[Vec<u8>]) -> bool {
        eq(self, bytes)
    }
}

impl<'a> PartialEq<[&'a [u8]]> for Raw {
    fn eq(&self, bytes: &[&[u8]]) -> bool {
        eq(self, bytes)
    }
}

impl PartialEq<[String]> for Raw {
    fn eq(&self, bytes: &[String]) -> bool {
        eq(self, bytes)
    }
}

impl<'a> PartialEq<[&'a str]> for Raw {
    fn eq(&self, bytes: &[&'a str]) -> bool {
        eq(self, bytes)
    }
}

impl PartialEq<[u8]> for Raw {
    fn eq(&self, bytes: &[u8]) -> bool {
        match self.0 {
            Lines::Empty => bytes.is_empty(),
            Lines::One(ref line) => line.as_ref() == bytes,
            Lines::Many(..) => false
        }
    }
}

impl PartialEq<str> for Raw {
    fn eq(&self, s: &str) -> bool {
        self == s.as_bytes()
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
        Raw::from(val.into_bytes())
    }
}

impl From<Vec<u8>> for Raw {
    #[inline]
    fn from(val: Vec<u8>) -> Raw {
        Raw(Lines::One(maybe_literal(val.into())))
    }
}

impl<'a> From<&'a str> for Raw {
    fn from(val: &'a str) -> Raw {
        Raw::from(val.as_bytes())
    }
}

impl<'a> From<&'a [u8]> for Raw {
    fn from(val: &'a [u8]) -> Raw {
        Raw(Lines::One(maybe_literal(val.into())))
    }
}

impl From<Bytes> for Raw {
    #[inline]
    fn from(val: Bytes) -> Raw {
        Raw(Lines::One(val))
    }
}

pub fn parsed(val: Bytes) -> Raw {
    Raw(Lines::One(From::from(val)))
}

pub fn push(raw: &mut Raw, val: Bytes) {
    raw.push_line(val);
}

pub fn new() -> Raw {
    Raw(Lines::Empty)
}

impl fmt::Debug for Lines {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Lines::Empty => f.pad("[]"),
            Lines::One(ref line) => fmt::Debug::fmt(&[line], f),
            Lines::Many(ref lines) => fmt::Debug::fmt(lines, f)
        }
    }
}

impl ::std::ops::Index<usize> for Raw {
    type Output = [u8];

    fn index(&self, idx: usize) -> &[u8] {
        match self.0 {
            Lines::Empty => panic!("index of out of bounds: {}", idx),
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
        fn maybe_literal(s: Cow<[u8]>) -> Bytes {
            match s.len() {
                $($len => {
                    $(
                    if s.as_ref() == $value {
                        return Bytes::from_static($value);
                    }
                    )+
                })+

                _ => ()
            }

            Bytes::from(s.into_owned())
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

pub struct RawLines<'a> {
    inner: &'a Lines,
    pos: usize,
}

impl<'a> fmt::Debug for RawLines<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("RawLines")
            .field(&self.inner)
            .finish()
    }
}

impl<'a> Iterator for RawLines<'a> {
    type Item = &'a [u8];

    #[inline]
    fn next(&mut self) -> Option<&'a [u8]> {
        let current_pos = self.pos;
        self.pos += 1;
        match *self.inner {
            Lines::Empty => None,
            Lines::One(ref line) => {
                if current_pos == 0 {
                    Some(line.as_ref())
                } else {
                    None
                }
            }
            Lines::Many(ref lines) => lines.get(current_pos).map(|l| l.as_ref()),
        }
    }
}
