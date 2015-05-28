use header::{Encoding, QualityItem};

header! {
    #[doc="`Accept-Encoding` header, defined in"]
    #[doc="[RFC7231](http://tools.ietf.org/html/rfc7231#section-5.3.4)"]
    #[doc=""]
    #[doc="The `Accept-Encoding` header field can be used by user agents to"]
    #[doc="indicate what response content-codings are"]
    #[doc="acceptable in the response.  An  `identity` token is used as a synonym"]
    #[doc="for \"no encoding\" in order to communicate when no encoding is"]
    #[doc="preferred."]
    #[doc=""]
    #[doc="# ABNF"]
    #[doc="```plain"]
    #[doc="Accept-Encoding  = #( codings [ weight ] )"]
    #[doc="codings          = content-coding / \"identity\" / \"*\""]
    #[doc="```"]
    #[doc=""]
    #[doc="# Example values"]
    #[doc="* `compress, gzip`"]
    #[doc="* ``"]
    #[doc="* `*`"]
    #[doc="* `compress;q=0.5, gzip;q=1`"]
    #[doc="* `gzip;q=1.0, identity; q=0.5, *;q=0`"]
    #[doc=""]
    #[doc="# Examples"]
    #[doc="```"]
    #[doc="use hyper::header::{Headers, AcceptEncoding, Encoding, qitem};"]
    #[doc=""]
    #[doc="let mut headers = Headers::new();"]
    #[doc="headers.set("]
    #[doc="    AcceptEncoding(vec![qitem(Encoding::Chunked)])"]
    #[doc=");"]
    #[doc="```"]
    #[doc="```"]
    #[doc="use hyper::header::{Headers, AcceptEncoding, Encoding, qitem};"]
    #[doc=" "]
    #[doc="let mut headers = Headers::new();"]
    #[doc="headers.set("]
    #[doc="    AcceptEncoding(vec!["]
    #[doc="        qitem(Encoding::Chunked),"]
    #[doc="        qitem(Encoding::Gzip),"]
    #[doc="        qitem(Encoding::Deflate),"]
    #[doc="    ])"]
    #[doc=");"]
    #[doc="```"]
    #[doc="```"]
    #[doc="use hyper::header::{Headers, AcceptEncoding, Encoding, QualityItem, Quality, qitem};"]
    #[doc=" "]
    #[doc="let mut headers = Headers::new();"]
    #[doc="headers.set("]
    #[doc="    AcceptEncoding(vec!["]
    #[doc="        qitem(Encoding::Chunked),"]
    #[doc="        QualityItem::new(Encoding::Gzip, Quality(600)),"]
    #[doc="        QualityItem::new(Encoding::EncodingExt(\"*\".to_owned()), Quality(0)),"]
    #[doc="    ])"]
    #[doc=");"]
    #[doc="```"]
    (AcceptEncoding, "Accept-Encoding") => (QualityItem<Encoding>)*

    test_accept_encoding {
        // From the RFC
        test_header!(test1, vec![b"compress, gzip"]);
        test_header!(test2, vec![b""], Some(AcceptEncoding(vec![])));
        test_header!(test3, vec![b"*"]);
        // Note: Removed quality 1 from gzip
        test_header!(test4, vec![b"compress;q=0.5, gzip"]);
        // Note: Removed quality 1 from gzip
        test_header!(test5, vec![b"gzip, identity; q=0.5, *;q=0"]);
    }
}
