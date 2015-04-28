use header::{Charset, QualityItem};

header! {
    #[doc="`Accept-Charset` header, defined in"]
    #[doc="[RFC7231](http://tools.ietf.org/html/rfc7231#section-5.3.3)"]
    #[doc=""]
    #[doc="The `Accept-Charset` header field can be sent by a user agent to"]
    #[doc="indicate what charsets are acceptable in textual response content."]
    #[doc="This field allows user agents capable of understanding more"]
    #[doc="comprehensive or special-purpose charsets to signal that capability"]
    #[doc="to an origin server that is capable of representing information in"]
    #[doc="those charsets."]
    #[doc=""]
    #[doc="# ABNF"]
    #[doc="```plain"]
    #[doc="Accept-Charset = 1#( ( charset / \"*\" ) [ weight ] )"]
    #[doc="```"]
    #[doc=""]
    #[doc="# Example values"]
    #[doc="* `iso-8859-5, unicode-1-1;q=0.8`"]
    (AcceptCharset, "Accept-Charset") => (QualityItem<Charset>)+

    test_accept_charset {
        /// Testcase from RFC
        test_header!(test1, vec![b"iso-8859-5, unicode-1-1;q=0.8"]);
    }
}
