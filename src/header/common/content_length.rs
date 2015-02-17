/// The `Content-Length` header.
///
/// Simply a wrapper around a `u64`.
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct ContentLength(pub u64);

impl_header!(ContentLength,
             "Content-Length",
             u64);

bench_header!(bench, ContentLength, { vec![b"42349984".to_vec()] });
