use std::cell::UnsafeCell;
use std::fmt;
use std::io::{self, Read};
use std::ops::{Deref, Range, RangeFrom, RangeTo, RangeFull};
use std::ptr;
use std::sync::Arc;

pub struct MemBuf {
    buf: Arc<UnsafeCell<Vec<u8>>>,
    start: usize,
    end: usize,
}

impl MemBuf {
    pub fn new() -> MemBuf {
        MemBuf::with_capacity(0)
    }

    pub fn with_capacity(cap: usize) -> MemBuf {
        MemBuf {
            buf: Arc::new(UnsafeCell::new(vec![0; cap])),
            start: 0,
            end: 0,
        }
    }

    pub fn bytes(&self) -> &[u8] {
        &self.buf()[self.start..self.end]
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn len(&self) -> usize {
        self.end - self.start
    }

    pub fn capacity(&self) -> usize {
        self.buf().len()
    }

    pub fn read_from<R: Read>(&mut self, io: &mut R) -> io::Result<usize> {
        let start = self.end - self.start;
        let n = try!(io.read(&mut self.buf_mut()[start..]));
        self.end += n;
        Ok(n)
    }

    pub fn slice(&mut self, len: usize) -> MemSlice {
        assert!(self.end - self.start >= len);
        let start = self.start;
        self.start += len;
        MemSlice {
            buf: self.buf.clone(),
            start: start,
            end: self.start,
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
        if is_unique && remaining + self.start >= needed {
            // we have unique access, we can mutate this vector
            trace!("MemBuf::reserve unique access, shifting");
            unsafe {
                let mut buf = &mut *self.buf.get();
                let len = self.len();
                ptr::copy(
                    buf.as_ptr().offset(self.start as isize),
                    buf.as_mut_ptr(),
                    len
                );
                self.start = 0;
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
            // we need to allocate more space, but dont have unique
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
                self.start = 0;
                self.end = 0;
            },
            None => {
                trace!("MemBuf::reset not unique, creating new MemBuf");
                *self = MemBuf::with_capacity(self.buf().len());
            }
        }
    }

    fn buf_mut(&mut self) -> &mut [u8] {
        // The contract here is that we NEVER have a MemSlice that exists
        // with slice.end > self.start.
        // In other words, we should *ALWAYS* be the only instance that can
        // look at the bytes on the right side of self.start.
        unsafe {
            &mut (*self.buf.get())[self.start..]
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
            .field("start", &self.start)
            .field("end", &self.end)
            .field("buf", &&self.buf()[self.start..self.end])
            .finish()
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

    pub fn slice<S: Slice>(&self, range: S) -> MemSlice {
        range.slice(self)
    }
}


impl fmt::Debug for MemSlice {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl Deref for  MemSlice {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        unsafe {
            &(*self.buf.get())[self.start..self.end]
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
        let mut buf = MemBuf::with_capacity(32);
        buf.read_from(&mut &b"Hello World"[..]).unwrap();

        let len = buf.len();
        let full = buf.slice(len);

        assert_eq!(&*full, b"Hello World");
        assert_eq!(&*full.slice(6..), b"World");
        assert_eq!(&*full.slice(..5), b"Hello");
        assert_eq!(&*full.slice(..), b"Hello World");
        for a in 0..len {
            for b in a..len {
                assert_eq!(&*full.slice(a..b), &b"Hello World"[a..b], "{}..{}", a, b);
            }
        }
    }
}
