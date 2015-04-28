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
    (AcceptEncoding, "Accept-Encoding") => (QualityItem<Encoding>)*

    test_accept_encoding {
        // From the RFC
        test_header!(test1, vec![b"compress, gzip"]);
        test_header!(test2, vec![b""]);
        test_header!(test3, vec![b"*"]);
        // Note: Removed quality 1 from gzip
        test_header!(test4, vec![b"compress;q=0.5, gzip"]);
        // Note: Removed quality 1 from gzip
        test_header!(test5, vec![b"gzip, identity; q=0.5, *;q=0"]);
    }
}
