use header::HttpDate;

header! {
    #[doc="`Expires` header, defined in [RFC7234](http://tools.ietf.org/html/rfc7234#section-5.3)"]
    #[doc=""]
    #[doc="The `Expires` header field gives the date/time after which the"]
    #[doc="response is considered stale."]
    #[doc=""]
    #[doc="The presence of an Expires field does not imply that the original"]
    #[doc="resource will change or cease to exist at, before, or after that"]
    #[doc="time."]
    #[doc=""]
    #[doc="# ABNF"]
    #[doc="```plain"]
    #[doc="Expires = HTTP-date"]
    #[doc="```"]
    #[doc=""]
    #[doc="# Example values"]
    #[doc="* `Thu, 01 Dec 1994 16:00:00 GMT`"]
    #[doc=""]
    #[doc="# Example"]
    #[doc="```"]
    #[doc="# extern crate hyper;"]
    #[doc="# extern crate time;"]
    #[doc="# fn main() {"]
    #[doc="// extern crate time;"]
    #[doc=""]
    #[doc="use hyper::header::{Headers, Expires, HttpDate};"]
    #[doc="use time::{self, Duration};"]
    #[doc=""]
    #[doc="let mut headers = Headers::new();"]
    #[doc="headers.set(Expires(HttpDate(time::now() + Duration::days(1))));"]
    #[doc="# }"]
    #[doc="```"]
    (Expires, "Expires") => [HttpDate]

    test_expires {
        // Testcase from RFC
        test_header!(test1, vec![b"Thu, 01 Dec 1994 16:00:00 GMT"]);
    }
}

bench_header!(imf_fixdate, Expires, { vec![b"Sun, 07 Nov 1994 08:48:37 GMT".to_vec()] });
bench_header!(rfc_850, Expires, { vec![b"Sunday, 06-Nov-94 08:49:37 GMT".to_vec()] });
bench_header!(asctime, Expires, { vec![b"Sun Nov  6 08:49:37 1994".to_vec()] });
