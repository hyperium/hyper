#![feature(test)]
#![deny(warnings)]

extern crate test;

use bytes::Buf;
use futures_util::stream;
use futures_util::StreamExt;
use http_body::Frame;
use http_body_util::{BodyExt, StreamBody};

macro_rules! bench_stream {
    ($bencher:ident, bytes: $bytes:expr, count: $count:expr, $total_ident:ident, $body_pat:pat, $block:expr) => {{
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("rt build");

        let $total_ident: usize = $bytes * $count;
        $bencher.bytes = $total_ident as u64;
        let __s: &'static [&'static [u8]] = &[&[b'x'; $bytes] as &[u8]; $count] as _;

        $bencher.iter(|| {
            rt.block_on(async {
                let $body_pat = StreamBody::new(
                    stream::iter(__s.iter())
                        .map(|&s| Ok::<_, std::convert::Infallible>(Frame::data(s))),
                );

                $block;
            });
        });
    }};
}

macro_rules! benches {
    ($($name:ident, $bytes:expr, $count:expr;)+) => (
        mod aggregate {
            use super::*;

            $(
            #[bench]
            fn $name(b: &mut test::Bencher) {
                bench_stream!(b, bytes: $bytes, count: $count, total, body, {
                    let buf = BodyExt::collect(body).await.unwrap().aggregate();
                    assert_eq!(buf.remaining(), total);
                });
            }
            )+
        }

        mod manual_into_vec {
            use super::*;

            $(
            #[bench]
            fn $name(b: &mut test::Bencher) {
                bench_stream!(b, bytes: $bytes, count: $count, total, mut body, {
                    let mut vec = Vec::new();
                    while let Some(chunk) = body.next().await {
                        vec.extend_from_slice(&chunk.unwrap().into_data().unwrap());
                    }
                    assert_eq!(vec.len(), total);
                });
            }
            )+
        }

        mod to_bytes {
            use super::*;

            $(
            #[bench]
            fn $name(b: &mut test::Bencher) {
                bench_stream!(b, bytes: $bytes, count: $count, total, body, {
                    let bytes = BodyExt::collect(body).await.unwrap().to_bytes();
                    assert_eq!(bytes.len(), total);
                });
            }
            )+
        }
    )
}

// ===== Actual Benchmarks =====

benches! {
    bytes_1_000_count_2, 1_000, 2;
    bytes_1_000_count_10, 1_000, 10;
    bytes_10_000_count_1, 10_000, 1;
    bytes_10_000_count_10, 10_000, 10;
}
