use std::borrow::Cow;
use std::cell::{Cell, UnsafeCell};
use std::fmt;
use std::io::{self, Read};
use std::ops::{Index, Range, RangeFrom, RangeTo, RangeFull};
use std::ptr;
use std::str;
use std::sync::Arc;

pub struct MemBuf {
    buf: Arc<UnsafeCell<Vec<u8>>>,
    start: Cell<usize>,
    end: usize,
}

impl MemBuf {
    pub fn new() -> MemBuf {
        MemBuf::with_capacity(0)
    }

    pub fn with_capacity(cap: usize) -> MemBuf {
        MemBuf {
            buf: Arc::new(UnsafeCell::new(vec![0; cap])),
            start: Cell::new(0),
            end: 0,
        }
    }

    pub fn bytes(&self) -> &[u8] {
        &self.buf()[self.start.get()..self.end]
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn len(&self) -> usize {
        self.end - self.start.get()
    }

    pub fn capacity(&self) -> usize {
        self.buf().len()
    }

    pub fn read_from<R: Read>(&mut self, io: &mut R) -> io::Result<usize> {
        let start = self.end - self.start.get();
        let n = try!(io.read(&mut self.buf_mut()[start..]));
        self.end += n;
        Ok(n)
    }

    pub fn slice(&self, len: usize) -> MemSlice {
        let start = self.start.get();
        assert!(!(self.end - start < len));
        let end = start + len;
        self.start.set(end);
        MemSlice {
            buf: self.buf.clone(),
            start: start,
            end: end,
        }
    }

    pub fn reserve(&mut self, needed: usize) {
        let orig_cap = self.capacity();
        let remaining = orig_cap - self.end;
        if remaining >= needed {
            // all done
            return
        }
        let is_unique = Arc::get_mut(&mut self.buf).is_some();
        trace!("MemBuf::reserve {} access", if is_unique { "unique" } else { "shared" });
        if is_unique && remaining + self.start.get() >= needed {
            // we have unique access, we can mutate this vector
            trace!("MemBuf::reserve unique access, shifting");
            unsafe {
                let mut buf = &mut *self.buf.get();
                let len = self.len();
                ptr::copy(
                    buf.as_ptr().offset(self.start.get() as isize),
                    buf.as_mut_ptr(),
                    len
                );
                self.start.set(0);
                self.end = len;
            }
        } else if is_unique {
            // we have unique access, we can mutate this vector
            trace!("MemBuf::reserve unique access, growing");
            unsafe {
                let mut vec = &mut *self.buf.get();
                grow_zerofill(vec, needed);
            }
        } else {
            // we need to allocate more space, but don't have unique
            // access, so we need to make a new buffer
            trace!("MemBuf::reserve shared buffer, creating new");
            let mut new = MemBuf::with_capacity(needed);
            unsafe {
                ptr::copy_nonoverlapping(
                    self.bytes().as_ptr(),
                    new.buf_mut().as_mut_ptr(),
                    self.len()
                );
            }
            new.end = self.len();
            *self = new;
        }
    }

    pub fn reset(&mut self) {
        match Arc::get_mut(&mut self.buf) {
            Some(_) => {
                trace!("MemBuf::reset was unique, re-using");
                self.start.set(0);
                self.end = 0;
            },
            None => {
                trace!("MemBuf::reset not unique, creating new MemBuf");
                *self = MemBuf::with_capacity(self.buf().len());
            }
        }
    }

    #[cfg(all(feature = "nightly", test))]
    pub fn restart(&mut self) {
        Arc::get_mut(&mut self.buf).unwrap();
        self.start.set(0);
    }

    fn buf_mut(&mut self) -> &mut [u8] {
        // The contract here is that we NEVER have a MemSlice that exists
        // with slice.end > self.start.
        // In other words, we should *ALWAYS* be the only instance that can
        // look at the bytes on the right side of self.start.
        unsafe {
            &mut (*self.buf.get())[self.start.get()..]
        }
    }

    fn buf(&self) -> &Vec<u8> {
        unsafe {
            &*self.buf.get()
        }
    }
}

#[inline]
unsafe fn grow_zerofill(buf: &mut Vec<u8>, additional: usize) {
    let orig_cap = buf.capacity();
    buf.reserve(additional);
    let new_cap = buf.capacity();
    let reserved = new_cap - orig_cap;
    let orig_len = buf.len();
    zero(buf, orig_len, reserved);
    buf.set_len(orig_len + reserved);


    unsafe fn zero(buf: &mut Vec<u8>, offset: usize, len: usize) {
        assert!(buf.capacity() >= len + offset,
            "offset of {} with len of {} is bigger than capacity of {}",
            offset, len, buf.capacity());
        ptr::write_bytes(buf.as_mut_ptr().offset(offset as isize), 0, len);
    }
}

#[test]
fn test_grow_zerofill() {
    for init in 0..100 {
        for reserve in (0..100).rev() {
            let mut vec = vec![0; init];
            unsafe { grow_zerofill(&mut vec, reserve) }
            assert_eq!(vec.len(), vec.capacity());
        }
    }
}

impl fmt::Debug for MemBuf {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("MemBuf")
            .field("start", &self.start.get())
            .field("end", &self.end)
            .field("buf", &&self.buf()[self.start.get()..self.end])
            .finish()
    }
}

impl From<Vec<u8>> for MemBuf {
    fn from(mut vec: Vec<u8>) -> MemBuf {
        let end = vec.iter().find(|&&x| x == 0).map(|&x| x as usize).unwrap_or(vec.len());
        vec.shrink_to_fit();
        MemBuf {
            buf: Arc::new(UnsafeCell::new(vec)),
            start: Cell::new(0),
            end: end,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemStr(MemSlice);

impl MemStr {
    pub unsafe fn from_utf8_unchecked(slice: MemSlice) -> MemStr {
        MemStr(slice)
    }

    pub fn as_str(&self) -> &str {
        unsafe { str::from_utf8_unchecked(self.0.as_ref()) }
    }
}

pub struct MemSlice {
    buf: Arc<UnsafeCell<Vec<u8>>>,
    start: usize,
    end: usize,
}

impl MemSlice {
    pub fn empty() -> MemSlice {
        MemSlice {
            buf: Arc::new(UnsafeCell::new(Vec::new())),
            start: 0,
            end: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.get().len()
    }

    pub fn is_empty(&self) -> bool {
        self.get().is_empty()
    }

    pub fn slice<S: Slice>(&self, range: S) -> MemSlice {
        range.slice(self)
    }

    fn get(&self) -> &[u8] {
        unsafe { &(*self.buf.get())[self.start..self.end] }
    }
}

impl AsRef<[u8]> for MemSlice {
    fn as_ref(&self) -> &[u8] {
        self.get()
    }
}

impl fmt::Debug for MemSlice {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&self.get(), f)
    }
}

impl Index<usize> for MemSlice {
    type Output = u8;
    fn index(&self, i: usize) -> &u8 {
        &self.get()[i]
    }
}

impl<'a> From<&'a [u8]> for MemSlice {
    fn from(v: &'a [u8]) -> MemSlice {
        MemSlice {
            buf: Arc::new(UnsafeCell::new(v.to_vec())),
            start: 0,
            end: v.len(),
        }
    }
}

impl From<Vec<u8>> for MemSlice {
    fn from(v: Vec<u8>) -> MemSlice {
        let len = v.len();
        MemSlice {
            buf: Arc::new(UnsafeCell::new(v)),
            start: 0,
            end: len,
        }
    }
}

impl<'a> From<&'a str> for MemSlice {
    fn from(v: &'a str) -> MemSlice {
        let v = v.as_bytes();
        MemSlice {
            buf: Arc::new(UnsafeCell::new(v.to_vec())),
            start: 0,
            end: v.len(),
        }
    }
}

impl<'a> From<Cow<'a, [u8]>> for MemSlice {
    fn from(v: Cow<'a, [u8]>) -> MemSlice {
        let v = v.into_owned();
        let len = v.len();
        MemSlice {
            buf: Arc::new(UnsafeCell::new(v)),
            start: 0,
            end: len,
        }
    }
}

impl PartialEq for MemSlice {
    fn eq(&self, other: &MemSlice) -> bool {
        self.get() == other.get()
    }
}

impl PartialEq<[u8]> for MemSlice {
    fn eq(&self, other: &[u8]) -> bool {
        self.get() == other
    }
}

impl PartialEq<str> for MemSlice {
    fn eq(&self, other: &str) -> bool {
        self.get() == other.as_bytes()
    }
}

impl PartialEq<Vec<u8>> for MemSlice {
    fn eq(&self, other: &Vec<u8>) -> bool {
        self.get() == other.as_slice()
    }
}

impl Eq for MemSlice {}

impl Clone for MemSlice {
    fn clone(&self) -> MemSlice {
        MemSlice {
            buf: self.buf.clone(),
            start: self.start,
            end: self.end,
        }
    }
}

pub trait Slice {
    fn slice(self, subject: &MemSlice) -> MemSlice;
}


impl Slice for Range<usize> {
    fn slice(self, subject: &MemSlice) -> MemSlice {
        assert!(subject.start + self.start <= subject.end);
        assert!(subject.start + self.end <= subject.end);
        MemSlice {
            buf: subject.buf.clone(),
            start: subject.start + self.start,
            end: subject.start + self.end,
        }
    }
}

impl Slice for RangeFrom<usize> {
    fn slice(self, subject: &MemSlice) -> MemSlice {
        assert!(subject.start + self.start <= subject.end);
        MemSlice {
            buf: subject.buf.clone(),
            start: subject.start + self.start,
            end: subject.end,
        }
    }
}

impl Slice for RangeTo<usize> {
    fn slice(self, subject: &MemSlice) -> MemSlice {
        assert!(subject.start + self.end <= subject.end);
        MemSlice {
            buf: subject.buf.clone(),
            start: subject.start,
            end: subject.start + self.end,
        }
    }
}

impl Slice for RangeFull {
    fn slice(self, subject: &MemSlice) -> MemSlice {
        MemSlice {
            buf: subject.buf.clone(),
            start: subject.start,
            end: subject.end,
        }
    }
}

unsafe impl Send for MemBuf {}
unsafe impl Send for MemSlice {}
unsafe impl Sync for MemSlice {}

#[cfg(test)]
impl<T: Read> ::http::io::MemRead for ::mock::AsyncIo<T> {
    fn read_mem(&mut self, len: usize) -> io::Result<MemSlice> {
        let mut v = vec![0; len];
        let n = try!(self.read(v.as_mut_slice()));
        v.truncate(n);
        Ok(MemSlice {
            buf: Arc::new(UnsafeCell::new(v)),
            start: 0,
            end: n,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{MemBuf};

    #[test]
    fn test_mem_slice_slice() {
        let buf = MemBuf::from(b"Hello World".to_vec());

        let len = buf.len();
        let full = buf.slice(len);

        assert_eq!(full.as_ref(), b"Hello World");
        assert_eq!(full.slice(6..).as_ref(), b"World");
        assert_eq!(full.slice(..5).as_ref(), b"Hello");
        assert_eq!(full.slice(..).as_ref(), b"Hello World");
        for a in 0..len {
            for b in a..len {
                assert_eq!(full.slice(a..b).as_ref(), &b"Hello World"[a..b], "{}..{}", a, b);
            }
        }
    }
}
