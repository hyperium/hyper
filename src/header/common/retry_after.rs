// Copyright (c) 2016 retry-after Developers
//
// This file is dual licensed under MIT and Apache 2.0
//
// *******************************************************
//
// Permission is hereby granted, free of charge, to any
// person obtaining a copy of this software and associated
// documentation files (the "Software"), to deal in the
// Software without restriction, including without
// limitation the rights to use, copy, modify, merge,
// publish, distribute, sublicense, and/or sell copies of
// the Software, and to permit persons to whom the Software
// is furnished to do so, subject to the following
//
// conditions:
//
// The above copyright notice and this permission notice
// shall be included in all copies or substantial portions
// of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF
// ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED
// TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A
// PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT
// SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
// CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
// OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR
// IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
// DEALINGS IN THE SOFTWARE.
//
// *******************************************************
//
// Apache License
// Version 2.0, January 2004
// http://www.apache.org/licenses/

use std::fmt;
use std::time::Duration;

use header::{Header, Raw};
use header::shared::HttpDate;

/// The `Retry-After` header.
///
/// The `Retry-After` response-header field can be used with a 503 (Service
/// Unavailable) response to indicate how long the service is expected to be
/// unavailable to the requesting client. This field MAY also be used with any
/// 3xx (Redirection) response to indicate the minimum time the user-agent is
/// asked wait before issuing the redirected request. The value of this field
/// can be either an HTTP-date or an integer number of seconds (in decimal)
/// after the time of the response.
///
/// # Examples
/// ```
/// use std::time::Duration;
/// use hyper::header::{Headers, RetryAfter};
///
/// let mut headers = Headers::new();
/// headers.set(
///     RetryAfter::Delay(Duration::from_secs(300))
/// );
/// ```
/// ```
/// use std::time::{SystemTime, Duration};
/// use hyper::header::{Headers, RetryAfter};
///
/// let mut headers = Headers::new();
/// let date = SystemTime::now() + Duration::from_secs(300);
/// headers.set(
///     RetryAfter::DateTime(date.into())
/// );
/// ```

/// Retry-After header, defined in [RFC7231](http://tools.ietf.org/html/rfc7231#section-7.1.3)
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum RetryAfter {
    /// Retry after this duration has elapsed
    ///
    /// This can be coupled with a response time header to produce a DateTime.
    Delay(Duration),

    /// Retry after the given DateTime
    DateTime(HttpDate),
}

impl Header for RetryAfter {
    fn header_name() -> &'static str {
        static NAME: &'static str = "Retry-After";
        NAME
    }

    fn parse_header(raw: &Raw) -> ::Result<RetryAfter> {
        if let Some(ref line) = raw.one() {
            let utf8_str = match ::std::str::from_utf8(line) {
                Ok(utf8_str) => utf8_str,
                Err(_) => return Err(::Error::Header),
            };

            if let Ok(datetime) = utf8_str.parse::<HttpDate>() {
                return Ok(RetryAfter::DateTime(datetime))
            }

            if let Ok(seconds) = utf8_str.parse::<u64>() {
                return Ok(RetryAfter::Delay(Duration::from_secs(seconds)));
            }

            Err(::Error::Header)
        } else {
            Err(::Error::Header)
        }
    }

    fn fmt_header(&self, f: &mut ::header::Formatter) -> ::std::fmt::Result {
        f.fmt_line(self)
    }
}

impl fmt::Display for RetryAfter {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            RetryAfter::Delay(ref duration) => {
                write!(f, "{}", duration.as_secs())
            },
            RetryAfter::DateTime(ref datetime) => {
                fmt::Display::fmt(datetime, f)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;
    use header::Header;
    use header::shared::HttpDate;

    use super::RetryAfter;

    #[test]
    fn header_name_regression() {
        assert_eq!(RetryAfter::header_name(), "Retry-After");
    }

    #[test]
    fn parse_delay() {
        let retry_after = RetryAfter::parse_header(&vec![b"1234".to_vec()].into()).unwrap();

        assert_eq!(RetryAfter::Delay(Duration::from_secs(1234)), retry_after);
    }

    macro_rules! test_retry_after_datetime {
        ($name:ident, $bytes:expr) => {
            #[test]
            fn $name() {
                let dt = "Sun, 06 Nov 1994 08:49:37 GMT".parse::<HttpDate>().unwrap();
                let retry_after = RetryAfter::parse_header(&vec![$bytes.to_vec()].into()).expect("parse_header ok");

                assert_eq!(RetryAfter::DateTime(dt), retry_after);
            }
        }
    }

    test_retry_after_datetime!(header_parse_rfc1123, b"Sun, 06 Nov 1994 08:49:37 GMT");
    test_retry_after_datetime!(header_parse_rfc850, b"Sunday, 06-Nov-94 08:49:37 GMT");
    test_retry_after_datetime!(header_parse_asctime, b"Sun Nov  6 08:49:37 1994");

    #[test]
    fn hyper_headers_from_raw_delay() {
        let retry_after = RetryAfter::parse_header(&b"300".to_vec().into()).unwrap();
        assert_eq!(retry_after, RetryAfter::Delay(Duration::from_secs(300)));
    }

    #[test]
    fn hyper_headers_from_raw_datetime() {
        let retry_after = RetryAfter::parse_header(&b"Sun, 06 Nov 1994 08:49:37 GMT".to_vec().into()).unwrap();
        let expected = "Sun, 06 Nov 1994 08:49:37 GMT".parse::<HttpDate>().unwrap();

        assert_eq!(retry_after, RetryAfter::DateTime(expected));
    }
}
