use bytes::Buf;
use iovec::IoVec;

/// A `Buf` wrapping a static byte slice.
#[derive(Debug)]
pub(crate) struct StaticBuf(pub(crate) &'static [u8]);

impl Buf for StaticBuf {
    #[inline]
    fn remaining(&self) -> usize {
        self.0.len()
    }

    #[inline]
    fn bytes(&self) -> &[u8] {
        self.0
    }

    #[inline]
    fn advance(&mut self, cnt: usize) {
        self.0 = &self.0[cnt..];
    }

    #[inline]
    fn bytes_vec<'t>(&'t self, dst: &mut [&'t IoVec]) -> usize {
        if dst.is_empty() || self.0.is_empty() {
            0
        } else {
            dst[0] = self.0.into();
            1
        }
    }
}

