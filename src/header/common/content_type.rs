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
    #[doc=""]
    #[doc="# Example values"]
    #[doc="* `text/html; charset=ISO-8859-4`"]
    (ContentType, "Content-Type") => [Mime]

    test_content_type {
        test_header!(
            test1,
            // FIXME: Should be b"text/html; charset=ISO-8859-4" but mime crate lowercases
            // the whole value so parsing and formatting the value gives a different result
            vec![b"text/html; charset=iso-8859-4"],
            Some(HeaderField(Mime(TopLevel::Text, SubLevel::Html, vec![(Attr::Charset, Value::Ext("iso-8859-4".to_string()))]))));
    }
}

bench_header!(bench, ContentType, { vec![b"application/json; charset=utf-8".to_vec()] });
