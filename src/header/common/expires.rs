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
    (Expires, "Expires") => [HttpDate]
}

bench_header!(imf_fixdate, Expires, { vec![b"Sun, 07 Nov 1994 08:48:37 GMT".to_vec()] });
bench_header!(rfc_850, Expires, { vec![b"Sunday, 06-Nov-94 08:49:37 GMT".to_vec()] });
bench_header!(asctime, Expires, { vec![b"Sun Nov  6 08:49:37 1994".to_vec()] });
