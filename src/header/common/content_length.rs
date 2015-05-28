header! {
    #[doc="`Content-Length` header, defined in"]
    #[doc="[RFC7230](http://tools.ietf.org/html/rfc7230#section-3.3.2)"]
    #[doc=""]
    #[doc="When a message does not have a `Transfer-Encoding` header field, a"]
    #[doc="Content-Length header field can provide the anticipated size, as a"]
    #[doc="decimal number of octets, for a potential payload body.  For messages"]
    #[doc="that do include a payload body, the Content-Length field-value"]
    #[doc="provides the framing information necessary for determining where the"]
    #[doc="body (and message) ends.  For messages that do not include a payload"]
    #[doc="body, the Content-Length indicates the size of the selected"]
    #[doc="representation."]
    #[doc=""]
    #[doc="# ABNF"]
    #[doc="```plain"]
    #[doc="Content-Length = 1*DIGIT"]
    #[doc="```"]
    #[doc=""]
    #[doc="# Example values"]
    #[doc="* `3495`"]
    #[doc=""]
    #[doc="# Example"]
    #[doc="```"]
    #[doc="use hyper::header::{Headers, ContentLength};"]
    #[doc=""]
    #[doc="let mut headers = Headers::new();"]
    #[doc="headers.set(ContentLength(1024u64));"]
    #[doc="```"]
    (ContentLength, "Content-Length") => [u64]

    test_content_length {
        // Testcase from RFC
        test_header!(test1, vec![b"3495"], Some(HeaderField(3495)));
    }
}

bench_header!(bench, ContentLength, { vec![b"42349984".to_vec()] });
