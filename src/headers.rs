#[cfg(feature = "http1")]
use bytes::BytesMut;
use http::header::CONTENT_LENGTH;
use http::header::{HeaderValue, ValueIter};
#[cfg(feature = "http2")]
#[cfg(feature = "client")]
use http::Method;
use http::HeaderMap;

#[cfg(feature = "http1")]
pub(super) fn connection_keep_alive(value: &HeaderValue) -> bool {
    connection_has(value, "keep-alive")
}

#[cfg(feature = "http1")]
pub(super) fn connection_close(value: &HeaderValue) -> bool {
    connection_has(value, "close")
}

#[cfg(feature = "http1")]
fn connection_has(value: &HeaderValue, needle: &str) -> bool {
    if let Ok(s) = value.to_str() {
        for val in s.split(',') {
            if val.trim().eq_ignore_ascii_case(needle) {
                return true;
            }
        }
    }
    false
}

#[cfg(feature = "http1")]
#[cfg(feature = "server")]
pub(super) fn content_length_parse(value: &HeaderValue) -> Option<u64> {
    value.to_str().ok().and_then(|s| s.parse().ok())
}

pub(super) fn content_length_parse_all(headers: &HeaderMap) -> Option<u64> {
    content_length_parse_all_values(headers.get_all(CONTENT_LENGTH).into_iter())
}

pub(super) fn content_length_parse_all_values(values: ValueIter<'_, HeaderValue>) -> Option<u64> {
    // If multiple Content-Length headers were sent, everything can still
    // be alright if they all contain the same value, and all parse
    // correctly. If not, then it's an error.

    let folded = values.fold(None, |prev, line| match prev {
        Some(Ok(prev)) => Some(
            line.to_str()
                .map_err(|_| ())
                .and_then(|s| s.parse().map_err(|_| ()))
                .and_then(|n| if prev == n { Ok(n) } else { Err(()) }),
        ),
        None => Some(
            line.to_str()
                .map_err(|_| ())
                .and_then(|s| s.parse().map_err(|_| ())),
        ),
        Some(Err(())) => Some(Err(())),
    });

    if let Some(Ok(n)) = folded {
        Some(n)
    } else {
        None
    }
}

#[cfg(feature = "http2")]
#[cfg(feature = "client")]
pub(super) fn method_has_defined_payload_semantics(method: &Method) -> bool {
    match *method {
        Method::GET | Method::HEAD | Method::DELETE | Method::CONNECT => false,
        _ => true,
    }
}

#[cfg(feature = "http2")]
pub(super) fn set_content_length_if_missing(headers: &mut HeaderMap, len: u64) {
    headers
        .entry(CONTENT_LENGTH)
        .or_insert_with(|| HeaderValue::from(len));
}

#[cfg(feature = "http1")]
pub(super) fn transfer_encoding_is_chunked(headers: &HeaderMap) -> bool {
    is_chunked(headers.get_all(http::header::TRANSFER_ENCODING).into_iter())
}

#[cfg(feature = "http1")]
pub(super) fn is_chunked(mut encodings: ValueIter<'_, HeaderValue>) -> bool {
    // chunked must always be the last encoding, according to spec
    if let Some(line) = encodings.next_back() {
        return is_chunked_(line);
    }

    false
}

#[cfg(feature = "http1")]
pub(super) fn is_chunked_(value: &HeaderValue) -> bool {
    // chunked must always be the last encoding, according to spec
    if let Ok(s) = value.to_str() {
        if let Some(encoding) = s.rsplit(',').next() {
            return encoding.trim().eq_ignore_ascii_case("chunked");
        }
    }

    false
}

#[cfg(feature = "http1")]
pub(super) fn add_chunked(mut entry: http::header::OccupiedEntry<'_, HeaderValue>) {
    const CHUNKED: &str = "chunked";

    if let Some(line) = entry.iter_mut().next_back() {
        // + 2 for ", "
        let new_cap = line.as_bytes().len() + CHUNKED.len() + 2;
        let mut buf = BytesMut::with_capacity(new_cap);
        buf.copy_from_slice(line.as_bytes());
        buf.copy_from_slice(b", ");
        buf.copy_from_slice(CHUNKED.as_bytes());

        *line = HeaderValue::from_maybe_shared(buf.freeze())
            .expect("original header value plus ascii is valid");
        return;
    }

    entry.insert(HeaderValue::from_static(CHUNKED));
}
