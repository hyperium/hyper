use header::HttpDate;

header! {
    #[doc="`Last-Modified` header, defined in"]
    #[doc="[RFC7232](http://tools.ietf.org/html/rfc7232#section-2.2)"]
    #[doc=""]
    #[doc="The `Last-Modified` header field in a response provides a timestamp"]
    #[doc="indicating the date and time at which the origin server believes the"]
    #[doc="selected representation was last modified, as determined at the"]
    #[doc="conclusion of handling the request."]
    #[doc=""]
    #[doc="# ABNF"]
    #[doc="```plain"]
    #[doc="Expires = HTTP-date"]
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
    #[doc="use hyper::header::{Headers, LastModified, HttpDate};"]
    #[doc="use time::{self, Duration};"]
    #[doc=""]
    #[doc="let mut headers = Headers::new();"]
    #[doc="headers.set(LastModified(HttpDate(time::now() - Duration::days(1))));"]
    #[doc="# }"]
    #[doc="```"]
    (LastModified, "Last-Modified") => [HttpDate]

    test_last_modified {
        // Testcase from RFC
        test_header!(test1, vec![b"Sat, 29 Oct 1994 19:43:31 GMT"]);}
}

bench_header!(imf_fixdate, LastModified, { vec![b"Sun, 07 Nov 1994 08:48:37 GMT".to_vec()] });
bench_header!(rfc_850, LastModified, { vec![b"Sunday, 06-Nov-94 08:49:37 GMT".to_vec()] });
bench_header!(asctime, LastModified, { vec![b"Sun Nov  6 08:49:37 1994".to_vec()] });
