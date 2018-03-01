use http::HeaderMap;
use http::header::{CONNECTION, CONTENT_LENGTH, EXPECT, HeaderValue, TRANSFER_ENCODING};
use unicase;

pub fn connection_keep_alive(headers: &HeaderMap) -> bool {
    for line in headers.get_all(CONNECTION) {
        if let Ok(s) = line.to_str() {
            for val in s.split(',') {
                if unicase::eq_ascii(val.trim(), "keep-alive") {
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
                if unicase::eq_ascii(val.trim(), "close") {
                    return true;
                }
            }
        }
    }

    false
}

pub fn content_length_parse(headers: &HeaderMap) -> Option<u64> {
    // If multiple Content-Length headers were sent, everything can still
    // be alright if they all contain the same value, and all parse
    // correctly. If not, then it's an error.

    let values = headers.get_all(CONTENT_LENGTH);
    let folded = values
        .into_iter()
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

pub fn expect_continue(headers: &HeaderMap) -> bool {
    Some(&b"100-continue"[..]) == headers.get(EXPECT).map(|v| v.as_bytes())
}

pub fn transfer_encoding_is_chunked(headers: &HeaderMap) -> bool {
    let mut encodings = headers.get_all(TRANSFER_ENCODING).into_iter();
    // chunked must always be the last encoding, according to spec
    if let Some(line) = encodings.next_back() {
        if let Ok(s) = line.to_str() {
            if let Some(encoding) = s.rsplit(',').next() {
                return unicase::eq_ascii(encoding.trim(), "chunked");
            }
        }
    }

    false
}
