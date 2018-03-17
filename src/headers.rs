use std::fmt::Write;

use bytes::BytesMut;
use http::HeaderMap;
use http::header::{CONNECTION, CONTENT_LENGTH, EXPECT, TRANSFER_ENCODING};
use http::header::{HeaderValue, OccupiedEntry, ValueIter};

/// Maximum number of bytes needed to serialize a u64 into ASCII decimal.
const MAX_DECIMAL_U64_BYTES: usize = 20;

pub fn connection_keep_alive(headers: &HeaderMap) -> bool {
    for line in headers.get_all(CONNECTION) {
        if let Ok(s) = line.to_str() {
            for val in s.split(',') {
                if eq_ascii(val.trim(), "keep-alive") {
                    return true;
                }
            }
        }
    }

    false
}

pub fn connection_close(headers: &HeaderMap) -> bool {
    for line in headers.get_all(CONNECTION) {
        if let Ok(s) = line.to_str() {
            for val in s.split(',') {
                if eq_ascii(val.trim(), "close") {
                    return true;
                }
            }
        }
    }

    false
}

pub fn content_length_parse(headers: &HeaderMap) -> Option<u64> {
    content_length_parse_all(headers.get_all(CONTENT_LENGTH).into_iter())
}

pub fn content_length_parse_all(values: ValueIter<HeaderValue>) -> Option<u64> {
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

pub fn content_length_zero(headers: &mut HeaderMap) {
    headers.insert(CONTENT_LENGTH, HeaderValue::from_static("0"));
}

pub fn content_length_value(len: u64) -> HeaderValue {
    let mut len_buf = BytesMut::with_capacity(MAX_DECIMAL_U64_BYTES);
    write!(len_buf, "{}", len)
        .expect("BytesMut can hold a decimal u64");
    // safe because u64 Display is ascii numerals
    unsafe {
        HeaderValue::from_shared_unchecked(len_buf.freeze())
    }
}

pub fn expect_continue(headers: &HeaderMap) -> bool {
    Some(&b"100-continue"[..]) == headers.get(EXPECT).map(|v| v.as_bytes())
}

pub fn transfer_encoding_is_chunked(headers: &HeaderMap) -> bool {
    is_chunked(headers.get_all(TRANSFER_ENCODING).into_iter())
}

pub fn is_chunked(mut encodings: ValueIter<HeaderValue>) -> bool {
    // chunked must always be the last encoding, according to spec
    if let Some(line) = encodings.next_back() {
        if let Ok(s) = line.to_str() {
            if let Some(encoding) = s.rsplit(',').next() {
                return eq_ascii(encoding.trim(), "chunked");
            }
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
    // Once our minimum Rust compiler version is >=1.23, this can be removed.
    #[allow(unused)]
    use std::ascii::AsciiExt;

    left.eq_ignore_ascii_case(right)
}

#[cfg(test)]
mod tests {
    #[test]
    fn assert_max_decimal_u64_bytes() {
        assert_eq!(
            super::MAX_DECIMAL_U64_BYTES,
            ::std::u64::MAX.to_string().len()
        );
    }
}
