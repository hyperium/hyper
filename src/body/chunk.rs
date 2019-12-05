use bytes::{Bytes};

/// A piece of a message body.
///
/// These are returned by [`Body`](::Body). It is an efficient buffer type.
///
/// A `Chunk` can be easily created by many of Rust's standard types that
/// represent a collection of bytes, using `Chunk::from`.
///
/// Compatibility note: in Hyper v0.13 this type was changed from a
/// newtype wrapper around `bytes::Bytes` to being merely a type alias.
/// In future releases this type may be removed entirely in favor of `Bytes`.
pub type Chunk = Bytes;

#[cfg(test)]
mod tests {
    #[cfg(feature = "nightly")]
    use test::Bencher;

    #[cfg(feature = "nightly")]
    #[bench]
    fn bench_chunk_static_buf(b: &mut Bencher) {
        use bytes::BufMut;

        let s = "Hello, World!";
        b.bytes = s.len() as u64;

        let mut dst = Vec::with_capacity(128);

        b.iter(|| {
            let chunk = crate::Chunk::from(s);
            dst.put(chunk);
            ::test::black_box(&dst);
            dst.clear();
        })
    }
}

