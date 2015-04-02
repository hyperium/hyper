use mime::Mime;

header! {
    #[doc="`Content-Type` header, defined in"]
    #[doc="[RFC7231](http://tools.ietf.org/html/rfc7231#section-3.1.1.5)"]
    #[doc=""]
    #[doc="The `Content-Type` header field indicates the media type of the"]
    #[doc="associated representation: either the representation enclosed in the"]
    #[doc="message payload or the selected representation, as determined by the"]
    #[doc="message semantics.  The indicated media type defines both the data"]
    #[doc="format and how that data is intended to be processed by a recipient,"]
    #[doc="within the scope of the received message semantics, after any content"]
    #[doc="codings indicated by Content-Encoding are decoded."]
    #[doc=""]
    #[doc="# ABNF"]
    #[doc="```plain"]
    #[doc="Content-Type = media-type"]
    #[doc="```"]
    (ContentType, "Content-Type") => [Mime]
}

bench_header!(bench, ContentType, { vec![b"application/json; charset=utf-8".to_vec()] });
