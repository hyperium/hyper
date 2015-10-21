use header::MediaType;
use header::media_types::type_::{APPLICATION, IMAGE, TEXT};

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
    #[doc=""]
    #[doc="# Examples"]
    #[doc="```"]
    #[doc="use hyper::header::{Headers, ContentType, MediaType};"]
    #[doc=""]
    #[doc="let mut headers = Headers::new();"]
    #[doc=""]
    #[doc="headers.set("]
    #[doc="    ContentType(MediaType::new(Some(\"text\"), None, Some(\"html\"), None))"]
    #[doc=");"]
    #[doc="```"]
    #[doc="```"]
    #[doc="use hyper::header::{Headers, ContentType};"]
    #[doc=""]
    #[doc="let mut headers = Headers::new();"]
    #[doc=""]
    #[doc="headers.set(ContentType::json());"]
    #[doc="```"]
    #[doc="```"]
    #[doc="use hyper::header::{ContentType, MediaType, Charset};"]
    #[doc=""]
    #[doc="let mut media_type = MediaType::new(Some(\"text\"), None, Some(\"html\"), None);"]
    #[doc="media_type.set_charset(Charset::Iso88594);"]
    #[doc="assert_eq!(media_type.to_string(), \"text/html; charset=ISO-8859-4\");"]
    #[doc="```"]
    (ContentType, "Content-Type") => [MediaType]

    test_content_type {}
}

impl ContentType {
    /// A constructor  to easily create a `Content-Type: application/json; charset=utf-8` header.
    #[inline]
    pub fn json() -> ContentType {
        let mut tag = MediaType::new(APPLICATION, None, Some("json"), None);
        tag.set_charset_utf8();
        ContentType(tag)
    }

    /// A constructor  to easily create a `Content-Type: text/plain; charset=utf-8` header.
    #[inline]
    pub fn plaintext() -> ContentType {
        let mut tag = MediaType::new(TEXT, None, Some("plain"), None);
        tag.set_charset_utf8();
        ContentType(tag)
    }

    /// A constructor  to easily create a `Content-Type: text/html; charset=utf-8` header.
    #[inline]
    pub fn html() -> ContentType {
        let mut tag = MediaType::new(TEXT, None, Some("html"), None);
        tag.set_charset_utf8();
        ContentType(tag)
    }

    /// A constructor  to easily create a `Content-Type: application/www-form-url-encoded` header.
    #[inline]
    pub fn form_url_encoded() -> ContentType {
        let mut tag = MediaType::new(APPLICATION, None, Some("www-form-url-encoded"), None);
        tag.set_charset_utf8();
        ContentType(tag)
    }
    /// A constructor  to easily create a `Content-Type: image/jpeg` header.
    #[inline]
    pub fn jpeg() -> ContentType {
        ContentType(MediaType::new(IMAGE, None, Some("jpeg"), None))
    }

    /// A constructor  to easily create a `Content-Type: image/png` header.
    #[inline]
    pub fn png() -> ContentType {
        ContentType(MediaType::new(IMAGE, None, Some("png"), None))
    }
}

bench_header!(bench, ContentType, { vec![b"application/json; charset=utf-8".to_vec()] });
