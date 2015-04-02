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
    (Date, "Date") => [HttpDate]
}

bench_header!(imf_fixdate, Date, { vec![b"Sun, 07 Nov 1994 08:48:37 GMT".to_vec()] });
bench_header!(rfc_850, Date, { vec![b"Sunday, 06-Nov-94 08:49:37 GMT".to_vec()] });
bench_header!(asctime, Date, { vec![b"Sun Nov  6 08:49:37 1994".to_vec()] });
