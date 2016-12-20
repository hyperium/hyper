use std::cmp;
use std::io::{self, Read, Write};
use std::ptr;


const INIT_BUFFER_SIZE: usize = 4096;
pub const MAX_BUFFER_SIZE: usize = 8192 + 4096 * 100;

#[derive(Debug, Default)]
pub struct Buffer {
    vec: Vec<u8>,
    tail: usize,
    head: usize,
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
        self.tail - self.head
    }

    #[inline]
    fn available(&self) -> usize {
        self.vec.len() - self.tail
    }

    #[inline]
    pub fn is_max_size(&self) -> bool {
        self.len() >= MAX_BUFFER_SIZE
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[inline]
    pub fn bytes(&self) -> &[u8] {
        &self.vec[self.head..self.tail]
    }

    #[inline]
    pub fn consume(&mut self, pos: usize) {
        debug_assert!(self.tail >= self.head + pos);
        self.head += pos;
        if self.head == self.tail {
            self.head = 0;
            self.tail = 0;
        }
    }

    pub fn consume_leading_lines(&mut self) {
        while !self.is_empty() {
            match self.vec[self.head] {
                b'\r' | b'\n' => {
                    self.consume(1);
                },
                _ => return
            }
        }
    }

    pub fn read_from<R: Read>(&mut self, r: &mut R) -> io::Result<usize> {
        self.maybe_reserve(1);
        let n = try!(r.read(&mut self.vec[self.tail..]));
        self.tail += n;
        self.maybe_reset();
        Ok(n)
    }

    pub fn write_into<W: Write>(&mut self, w: &mut W) -> io::Result<usize> {
        if self.is_empty() {
            Ok(0)
        } else {
            let n = try!(w.write(&mut self.vec[self.head..self.tail]));
            self.head += n;
            self.maybe_reset();
            Ok(n)
        }
    }

    pub fn write(&mut self, data: &[u8]) -> usize {
        trace!("Buffer::write len = {:?}", data.len());
        self.maybe_reserve(data.len());
        let len = cmp::min(self.available(), data.len());
        assert!(self.available() >= len);
        unsafe {
            // in rust 1.9, we could use slice::copy_from_slice
            ptr::copy(
                data.as_ptr(),
                self.vec.as_mut_ptr().offset(self.tail as isize),
                len
            );
        }
        self.tail += len;
        len
    }

    #[inline]
    fn maybe_reserve(&mut self, needed: usize) {
        let cap = self.vec.len();
        if cap == 0 {
            // first reserve
            let init = cmp::max(INIT_BUFFER_SIZE, needed);
            trace!("reserving initial {}", init);
            self.vec = vec![0; init];
        } else if self.head > 0  && self.tail == cap && self.head >= needed {
            // there is space to shift over
            let count = self.tail - self.head;
            trace!("moving buffer bytes over by {}", count);
            unsafe {
                ptr::copy(
                    self.vec.as_ptr().offset(self.head as isize),
                    self.vec.as_mut_ptr(),
                    count
                );
            }
            self.tail -= count;
            self.head = 0;
        } else if self.tail == cap && cap < MAX_BUFFER_SIZE {
            self.vec.reserve(cmp::min(cap * 4, MAX_BUFFER_SIZE) - cap);
            let new = self.vec.capacity() - cap;
            trace!("reserved {}", new);
            unsafe { grow_zerofill(&mut self.vec, new) }
        }
    }

    #[inline]
    fn maybe_reset(&mut self) {
        if self.tail != 0 && self.tail == self.head {
            self.tail = 0;
            self.head = 0;
        }
    }
}

#[inline]
unsafe fn grow_zerofill(buf: &mut Vec<u8>, additional: usize) {
    let len = buf.len();
    buf.set_len(len + additional);
    ptr::write_bytes(buf.as_mut_ptr(), 0, buf.len());
}
