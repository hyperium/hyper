use header::HttpDate;

header! {
    #[doc="`Date` header, defined in [RFC7231](http://tools.ietf.org/html/rfc7231#section-7.1.1.2)"]
    #[doc=""]
    #[doc="The `Date` header field represents the date and time at which the"]
    #[doc="message was originated."]
    #[doc=""]
    #[doc="# ABNF"]
    #[doc="```plain"]
    #[doc="Date = HTTP-date"]
    #[doc="```"]
    #[doc=""]
    #[doc="# Example values"]
    #[doc="* `Tue, 15 Nov 1994 08:12:31 GMT`"]
    #[doc=""]
    #[doc="# Example"]
    #[doc="```"]
    #[doc="# extern crate time;"]
    #[doc="# extern crate hyper;"]
    #[doc="# fn main() {"]
    #[doc="// extern crate time;"]
    #[doc=""]
    #[doc="use hyper::header::{Headers, Date, HttpDate};"]
    #[doc="use time;"]
    #[doc=""]
    #[doc="let mut headers = Headers::new();"]
    #[doc="headers.set(Date(HttpDate(time::now())));"]
    #[doc="# }"]
    #[doc="```"]
    (Date, "Date") => [HttpDate]

    test_date {
        test_header!(test1, vec![b"Tue, 15 Nov 1994 08:12:31 GMT"]);
    }
}

bench_header!(imf_fixdate, Date, { vec![b"Sun, 07 Nov 1994 08:48:37 GMT".to_vec()] });
bench_header!(rfc_850, Date, { vec![b"Sunday, 06-Nov-94 08:49:37 GMT".to_vec()] });
bench_header!(asctime, Date, { vec![b"Sun Nov  6 08:49:37 1994".to_vec()] });
