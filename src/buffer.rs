use std::cmp;
use std::iter;
use std::io::{self, Read, BufRead, Cursor};

pub struct BufReader<R> {
    buf: Cursor<Vec<u8>>,
    inner: R
}

const INIT_BUFFER_SIZE: usize = 4096;
const MAX_BUFFER_SIZE: usize = 8192 + 4096 * 100;

impl<R: Read> BufReader<R> {
    pub fn new(rdr: R) -> BufReader<R> {
        BufReader::with_capacity(rdr, INIT_BUFFER_SIZE)
    }

    pub fn with_capacity(rdr: R, cap: usize) -> BufReader<R> {
        BufReader {
            buf: Cursor::new(Vec::with_capacity(cap)),
            inner: rdr
        }
    }

    pub fn get_ref(&self) -> &R { &self.inner }

    pub fn get_mut(&mut self) -> &mut R { &mut self.inner }

    pub fn get_buf(&self) -> &[u8] {
        self.buf.get_ref()
    }

    pub fn into_inner(self) -> R { self.inner }

    pub fn read_into_buf(&mut self) -> io::Result<usize> {
        let v = self.buf.get_mut();
        reserve(v);
        let inner = &mut self.inner;
        with_end_to_cap(v, |b| inner.read(b))
    }
}

impl<R: Read> Read for BufReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.buf.get_ref().len() == self.buf.position() as usize &&
            buf.len() >= self.buf.get_ref().capacity() {
            return self.inner.read(buf);
        }
        try!(self.fill_buf());
        self.buf.read(buf)
    }
}

impl<R: Read> BufRead for BufReader<R> {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
         if self.buf.position() as usize == self.buf.get_ref().len() {
            self.buf.set_position(0);
            let v = self.buf.get_mut();
            v.truncate(0);
            let inner = &mut self.inner;
            try!(with_end_to_cap(v, |b| inner.read(b)));
         }
         self.buf.fill_buf()
    }

    fn consume(&mut self, amt: usize) {
        self.buf.consume(amt)
    }
}

fn with_end_to_cap<F>(v: &mut Vec<u8>, f: F) -> io::Result<usize>
    where F: FnOnce(&mut [u8]) -> io::Result<usize>
{
    let len = v.len();
    let new_area = v.capacity() - len;
    v.extend(iter::repeat(0).take(new_area));
    match f(&mut v[len..]) {
        Ok(n) => {
            v.truncate(len + n);
            Ok(n)
        }
        Err(e) => {
            v.truncate(len);
            Err(e)
        }
    }
}

#[inline]
fn reserve(v: &mut Vec<u8>) {
    let cap = v.capacity();
    if v.len() == cap {
        v.reserve(cmp::min(cap * 4, MAX_BUFFER_SIZE) - cap);
    }
}
