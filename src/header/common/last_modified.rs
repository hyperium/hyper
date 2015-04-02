use header::HttpDate;

header! {
    #[doc="`Last-Modified` header, defined in [RFC7232](http://tools.ietf.org/html/rfc7232#section-2.2)"]
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
    (LastModified, "Last-Modified") => [HttpDate]
}

bench_header!(imf_fixdate, LastModified, { vec![b"Sun, 07 Nov 1994 08:48:37 GMT".to_vec()] });
bench_header!(rfc_850, LastModified, { vec![b"Sunday, 06-Nov-94 08:49:37 GMT".to_vec()] });
bench_header!(asctime, LastModified, { vec![b"Sun Nov  6 08:49:37 1994".to_vec()] });
