use std::cmp;
use std::io::{self, Read};
use std::ptr;


const INIT_BUFFER_SIZE: usize = 4096;
const MAX_BUFFER_SIZE: usize = 8192 + 4096 * 100;

#[derive(Debug, Default)]
pub struct Buffer {
    vec: Vec<u8>,
    read_pos: usize,
    write_pos: usize,
}

impl Buffer {
    pub fn new() -> Buffer {
        Buffer::default()
    }

    pub fn reset(&mut self) {
        *self = Buffer::new()
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.read_pos - self.write_pos
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[inline]
    pub fn bytes(&self) -> &[u8] {
        &self.vec[self.write_pos..self.read_pos]
    }

    #[inline]
    pub fn consume(&mut self, pos: usize) {
        debug_assert!(self.read_pos >= self.write_pos + pos);
        self.write_pos += pos;
        if self.write_pos == self.read_pos {
            self.write_pos = 0;
            self.read_pos = 0;
        }
    }

    pub fn read_from<R: Read>(&mut self, r: &mut R) -> io::Result<usize> {
        self.maybe_reserve();
        let n = try!(r.read(&mut self.vec[self.read_pos..]));
        self.read_pos += n;
        Ok(n)
    }

    #[inline]
    fn maybe_reserve(&mut self) {
        let cap = self.vec.len();
        if cap == 0 {
            trace!("reserving initial {}", INIT_BUFFER_SIZE);
            self.vec = vec![0; INIT_BUFFER_SIZE];
        } else if self.write_pos > 0  && self.read_pos == cap {
            let count = self.read_pos - self.write_pos;
            trace!("moving buffer bytes over by {}", count);
            unsafe {
                ptr::copy(
                    self.vec.as_ptr().offset(self.write_pos as isize),
                    self.vec.as_mut_ptr(),
                    count
                );
            }
            self.read_pos -= count;
            self.write_pos = 0;
        } else if self.read_pos == cap && cap < MAX_BUFFER_SIZE {
            self.vec.reserve(cmp::min(cap * 4, MAX_BUFFER_SIZE) - cap);
            let new = self.vec.capacity() - cap;
            trace!("reserved {}", new);
            unsafe { grow_zerofill(&mut self.vec, new) }
        }
    }

    pub fn wrap<'a, 'b: 'a, R: io::Read>(&'a mut self, reader: &'b mut R) -> BufReader<'a, R> {
        BufReader {
            buf: self,
            reader: reader
        }
    }
}

#[derive(Debug)]
pub struct BufReader<'a, R: io::Read + 'a> {
    buf: &'a mut Buffer,
    reader: &'a mut R
}

impl<'a, R: io::Read + 'a> BufReader<'a, R> {
    pub fn get_ref(&self) -> &R {
        self.reader
    }
}

impl<'a, R: io::Read> Read for BufReader<'a, R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        trace!("BufReader.read self={}, buf={}", self.buf.len(), buf.len());
        let n = try!(self.buf.bytes().read(buf));
        self.buf.consume(n);
        if n == 0 {
            self.buf.reset();
            self.reader.read(&mut buf[n..])
        } else {
            Ok(n)
        }
    }
}

#[inline]
unsafe fn grow_zerofill(buf: &mut Vec<u8>, additional: usize) {
    let len = buf.len();
    buf.set_len(len + additional);
    ptr::write_bytes(buf.as_mut_ptr(), 0, buf.len());
}
