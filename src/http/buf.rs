use std::cell::UnsafeCell;
use std::fmt;
use std::io::{self, Read};
use std::sync::Arc;
#[cfg(test)]
use http::io::MemRead;
#[cfg(test)]
use std::convert::AsRef;
#[cfg(test)]
use mock::AsyncIo;

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
        self.len() != 0
    }

    pub fn len(&self) -> usize {
        self.end - self.start
    }

    pub fn capacity(&self) -> usize {
        self.buf().len()
    }

    pub fn read_from<R: Read>(&mut self, io: &mut R) -> io::Result<usize> {
        let end = self.end;
        trace!("read_from len = {}", self.buf_mut().len() - end);
        let n = try!(io.read(&mut self.buf_mut()[end..]));
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
        let left = orig_cap - self.end;
        if needed > left {
            if Arc::get_mut(&mut self.buf).is_some() {
                // we have unique access, we can mutate this vector
                trace!("MemBuf::reserve unique access, growing");
                unsafe {
                    let mut vec = &mut *self.buf.get();
                    vec.reserve(needed);
                    let new_cap = vec.capacity();
                    grow_zerofill(vec, new_cap - orig_cap);
                }
            } else {
                // we need to allocate more space, but dont have unique
                // access, so we need to make a new buffer
                trace!("MemBuf::reserve shared buffer, creating new");
                //TODO: copy [self.start..self.end]
                *self = MemBuf::with_capacity(needed);
            }
        }
    }

    fn reset(&mut self) {
        match Arc::get_mut(&mut self.buf) {
            Some(_) => {
                trace!("MemBuf::reset was unique, re-using");
                self.start = 0;
                self.end = 0;
            },
            None => {
                trace!("MemBuf::reset not unique, creating new MemBuf");
                *self = MemBuf::new();
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
    let len = buf.len();
    buf.set_len(len + additional);
    ::std::ptr::write_bytes(buf.as_mut_ptr(), 0, buf.len());
}

impl fmt::Debug for MemBuf {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&self.buf()[self.start..self.end], f)
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
}

#[cfg(test)]
impl MemRead for [u8] {
    fn read_mem(&mut self, len: usize) -> io::Result<MemSlice> {
        if self.len() > 0 {
            let n = ::std::cmp::min(len, self.len());
            Ok(MemSlice {
                buf: Arc::new(UnsafeCell::new(self[0..n].to_vec())),
                start: 0,
                end: n,
            })
        } else {
            Ok(MemSlice::empty())
        }
    }
}

#[cfg(test)]
impl<'a> MemRead for &'a [u8] {
    fn read_mem(&mut self, len: usize) -> io::Result<MemSlice> {
        if self.len() > 0 {
            let n = ::std::cmp::min(len, self.len());
            Ok(MemSlice {
                buf: Arc::new(UnsafeCell::new(self[0..n].to_vec())),
                start: 0,
                end: n,
            })
        } else {
            Ok(MemSlice::empty())
        }
    }
}

#[cfg(test)]
impl<T: Read + AsRef<[u8]>> MemRead for ::std::io::Cursor<T> {
    fn read_mem(&mut self, len: usize) -> io::Result<MemSlice> {
        let mut v = vec![0; len];
        match self.read(v.as_mut_slice()) {
            Ok(count) => {
                Ok(MemSlice {
                    buf: Arc::new(UnsafeCell::new(v[0..count].to_vec())),
                    start: 0,
                    end: count,
                })
            }
            _ => {
                Ok(MemSlice::empty())
            }
        }
    }
}

#[cfg(test)]
impl<T: Read> MemRead for AsyncIo<T> {
    fn read_mem(&mut self, len: usize) -> io::Result<MemSlice> {
        let mut v = vec![0; len];
        match self.read(v.as_mut_slice()) {
            Ok(count) => {
                Ok(MemSlice {
                    buf: Arc::new(UnsafeCell::new(v[0..count].to_vec())),
                    start: 0,
                    end: count,
                })
            }
            _ => {
                Ok(MemSlice::empty())
            }
        }
    }
}

impl fmt::Debug for MemSlice {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl ::std::ops::Deref for  MemSlice {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        unsafe {
            &(*self.buf.get())[self.start..self.end]
        }
    }
}

unsafe impl Send for MemBuf {}
unsafe impl Send for MemSlice {}
