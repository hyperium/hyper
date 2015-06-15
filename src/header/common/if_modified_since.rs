use header::HttpDate;

header! {
    #[doc="`If-Modified-Since` header, defined in"]
    #[doc="[RFC7232](http://tools.ietf.org/html/rfc7232#section-3.3)"]
    #[doc=""]
    #[doc="The `If-Modified-Since` header field makes a GET or HEAD request"]
    #[doc="method conditional on the selected representation's modification date"]
    #[doc="being more recent than the date provided in the field-value."]
    #[doc="Transfer of the selected representation's data is avoided if that"]
    #[doc="data has not changed."]
    #[doc=""]
    #[doc="# ABNF"]
    #[doc="```plain"]
    #[doc="If-Unmodified-Since = HTTP-date"]
    #[doc="```"]
    #[doc=""]
    #[doc="# Example values"]
    #[doc="* `Sat, 29 Oct 1994 19:43:31 GMT`"]
    #[doc=""]
    #[doc="# Example"]
    #[doc="```"]
    #[doc="# extern crate hyper;"]
    #[doc="# extern crate time;"]
    #[doc="# fn main() {"]
    #[doc="// extern crate time;"]
    #[doc=""]
    #[doc="use hyper::header::{Headers, IfModifiedSince, HttpDate};"]
    #[doc="use time::{self, Duration};"]
    #[doc=""]
    #[doc="let mut headers = Headers::new();"]
    #[doc="headers.set(IfModifiedSince(HttpDate(time::now() - Duration::days(1))));"]
    #[doc="# }"]
    #[doc="```"]
    (IfModifiedSince, "If-Modified-Since") => [HttpDate]

    test_if_modified_since {
        // Testcase from RFC
        test_header!(test1, vec![b"Sat, 29 Oct 1994 19:43:31 GMT"]);
    }
}

bench_header!(imf_fixdate, IfModifiedSince, { vec![b"Sun, 07 Nov 1994 08:48:37 GMT".to_vec()] });
bench_header!(rfc_850, IfModifiedSince, { vec![b"Sunday, 06-Nov-94 08:49:37 GMT".to_vec()] });
bench_header!(asctime, IfModifiedSince, { vec![b"Sun Nov  6 08:49:37 1994".to_vec()] });
