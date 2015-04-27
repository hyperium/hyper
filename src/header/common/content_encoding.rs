use header::Encoding;

header! {
    #[doc="`Content-Encoding` header, defined in"]
    #[doc="[RFC7231](http://tools.ietf.org/html/rfc7231#section-3.1.2.2)"]
    #[doc=""]
    #[doc="The `Content-Encoding` header field indicates what content codings"]
    #[doc="have been applied to the representation, beyond those inherent in the"]
    #[doc="media type, and thus what decoding mechanisms have to be applied in"]
    #[doc="order to obtain data in the media type referenced by the Content-Type"]
    #[doc="header field.  Content-Encoding is primarily used to allow a"]
    #[doc="representation's data to be compressed without losing the identity of"]
    #[doc="its underlying media type."]
    #[doc=""]
    #[doc="# ABNF"]
    #[doc="```plain"]
    #[doc="Content-Encoding = 1#content-coding"]
    #[doc="```"]
    (ContentEncoding, "Content-Encoding") => (Encoding)+

    test_content_encoding {}
}

bench_header!(single, ContentEncoding, { vec![b"gzip".to_vec()] });
bench_header!(multiple, ContentEncoding, { vec![b"gzip, deflate".to_vec()] });
