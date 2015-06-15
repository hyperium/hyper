use header::HttpDate;

header! {
    #[doc="`If-Unmodified-Since` header, defined in"]
    #[doc="[RFC7232](http://tools.ietf.org/html/rfc7232#section-3.4)"]
    #[doc=""]
    #[doc="The `If-Unmodified-Since` header field makes the request method"]
    #[doc="conditional on the selected representation's last modification date"]
    #[doc="being earlier than or equal to the date provided in the field-value."]
    #[doc="This field accomplishes the same purpose as If-Match for cases where"]
    #[doc="the user agent does not have an entity-tag for the representation."]
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
    #[doc="use hyper::header::{Headers, IfUnmodifiedSince, HttpDate};"]
    #[doc="use time::{self, Duration};"]
    #[doc=""]
    #[doc="let mut headers = Headers::new();"]
    #[doc="headers.set(IfUnmodifiedSince(HttpDate(time::now() - Duration::days(1))));"]
    #[doc="# }"]
    #[doc="```"]
    (IfUnmodifiedSince, "If-Unmodified-Since") => [HttpDate]

    test_if_unmodified_since {
        // Testcase from RFC
        test_header!(test1, vec![b"Sat, 29 Oct 1994 19:43:31 GMT"]);
    }
}

bench_header!(imf_fixdate, IfUnmodifiedSince, { vec![b"Sun, 07 Nov 1994 08:48:37 GMT".to_vec()] });
bench_header!(rfc_850, IfUnmodifiedSince, { vec![b"Sunday, 06-Nov-94 08:49:37 GMT".to_vec()] });
bench_header!(asctime, IfUnmodifiedSince, { vec![b"Sun Nov  6 08:49:37 1994".to_vec()] });
