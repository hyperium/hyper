use std::mem;

use bytes::BytesMut;
use http::HeaderMap;
use http::header::{CONTENT_LENGTH, TRANSFER_ENCODING};
use http::header::{HeaderValue, OccupiedEntry, ValueIter};

/// Maximum number of bytes needed to serialize a u64 into ASCII decimal.
const MAX_DECIMAL_U64_BYTES: usize = 20;

pub fn connection_keep_alive(value: &HeaderValue) -> bool {
    connection_has(value, "keep-alive")
}

pub fn connection_close(value: &HeaderValue) -> bool {
    connection_has(value, "close")
}

fn connection_has(value: &HeaderValue, needle: &str) -> bool {
    if let Ok(s) = value.to_str() {
        for val in s.split(',') {
            if eq_ascii(val.trim(), needle) {
                return true;
            }
        }
    }
    false
}

pub fn content_length_parse(value: &HeaderValue) -> Option<u64> {
    value
        .to_str()
        .ok()
        .and_then(|s| s.parse().ok())
}

pub fn content_length_parse_all(headers: &HeaderMap) -> Option<u64> {
    content_length_parse_all_values(headers.get_all(CONTENT_LENGTH).into_iter())
}

pub fn content_length_parse_all_values(values: ValueIter<HeaderValue>) -> Option<u64> {
    // If multiple Content-Length headers were sent, everything can still
    // be alright if they all contain the same value, and all parse
    // correctly. If not, then it's an error.

    let folded = values
        .fold(None, |prev, line| match prev {
            Some(Ok(prev)) => {
                Some(line
                    .to_str()
                    .map_err(|_| ())
                    .and_then(|s| s.parse().map_err(|_| ()))
                    .and_then(|n| if prev == n { Ok(n) } else { Err(()) }))
            },
            None => {
                Some(line
                    .to_str()
                    .map_err(|_| ())
                    .and_then(|s| s.parse().map_err(|_| ())))
            },
            Some(Err(())) => Some(Err(())),
        });

    if let Some(Ok(n)) = folded {
        Some(n)
    } else {
        None
    }
}

pub fn content_length_value(len: u64) -> HeaderValue {
    let mut len_buf = if mem::size_of::<BytesMut>() - 1 < MAX_DECIMAL_U64_BYTES {
        // when ptr size is 32bits...
        // max bytes don't fit inline, but the likelihood of the length
        // actually needing that many bytes is super tiny. So, only
        // allocate if we really need to.
        // constant version of 10.pow(15) - 1...
        const MAX_DECIMAL_INLINE: u64 = 999_999_999_999_999;
        if len <= MAX_DECIMAL_INLINE {
            // Still fits inline of 15 bytes...
            BytesMut::new()
        } else {
            // the number is huge, and doesn't fit into 15 bytes (on 32bit),
            // so we gotta allocate ;_;
            BytesMut::with_capacity(MAX_DECIMAL_U64_BYTES)
        }
    } else {
        // max bytes fit inline
        BytesMut::with_capacity(MAX_DECIMAL_U64_BYTES)
    };
    ::itoa::fmt(&mut len_buf, len)
        .expect("BytesMut can hold a decimal u64");
    // safe because u64 Display is ascii numerals
    unsafe {
        HeaderValue::from_shared_unchecked(len_buf.freeze())
    }
}

pub fn set_content_length_if_missing(headers: &mut HeaderMap, len: u64) {
    headers
        .entry(CONTENT_LENGTH)
        .unwrap()
        .or_insert(content_length_value(len));
}

pub fn transfer_encoding_is_chunked(headers: &HeaderMap) -> bool {
    is_chunked(headers.get_all(TRANSFER_ENCODING).into_iter())
}

pub fn is_chunked(mut encodings: ValueIter<HeaderValue>) -> bool {
    // chunked must always be the last encoding, according to spec
    if let Some(line) = encodings.next_back() {
        return is_chunked_(line);
    }

    false
}

pub fn is_chunked_(value: &HeaderValue) -> bool {
    // chunked must always be the last encoding, according to spec
    if let Ok(s) = value.to_str() {
        if let Some(encoding) = s.rsplit(',').next() {
            return eq_ascii(encoding.trim(), "chunked");
        }
    }

    false
}

pub fn add_chunked(mut entry: OccupiedEntry<HeaderValue>) {
    const CHUNKED: &'static str = "chunked";

    if let Some(line) = entry.iter_mut().next_back() {
        // + 2 for ", "
        let new_cap = line.as_bytes().len() + CHUNKED.len() + 2;
        let mut buf = BytesMut::with_capacity(new_cap);
        buf.copy_from_slice(line.as_bytes());
        buf.copy_from_slice(b", ");
        buf.copy_from_slice(CHUNKED.as_bytes());

        *line = HeaderValue::from_shared(buf.freeze())
            .expect("original header value plus ascii is valid");
        return;
    }

    entry.insert(HeaderValue::from_static(CHUNKED));
}

fn eq_ascii(left: &str, right: &str) -> bool {
    // As of Rust 1.23, str gained this method inherently, and so the
    // compiler says this trait is unused.
    //
    // TODO: Once our minimum Rust compiler version is >=1.23, this can be removed.
    #[allow(unused, deprecated)]
    use std::ascii::AsciiExt;

    left.eq_ignore_ascii_case(right)
}

#[cfg(test)]
mod tests {

    #[cfg(feature = "nightly")]
    use test::Bencher;

    #[test]
    fn assert_max_decimal_u64_bytes() {
        assert_eq!(
            super::MAX_DECIMAL_U64_BYTES,
            ::std::u64::MAX.to_string().len()
        );
    }

    #[cfg(feature = "nightly")]
    #[bench]
    fn bench_content_length_fmt_small(b: &mut Bencher) {
        let n = 13;
        let s = n.to_string();
        b.bytes = s.len() as u64;

        b.iter(|| {
            let val = super::content_length_value(n);
            debug_assert_eq!(val, s);
            ::test::black_box(&val);
        })
    }

    #[cfg(feature = "nightly")]
    #[bench]
    fn bench_content_length_fmt_big(b: &mut Bencher) {
        let n = 326_893_010;
        let s = n.to_string();
        b.bytes = s.len() as u64;

        b.iter(|| {
            let val = super::content_length_value(n);
            debug_assert_eq!(val, s);
            ::test::black_box(&val);
        })
    }
}
