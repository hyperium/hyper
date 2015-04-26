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
    (AcceptCharset, "Accept-Charset") => (QualityItem<Charset>)+

    test_accept_charset {
        test_header!(test1, vec![b"iso-8859-5, unicode-1-1;q=0.8"]);
    }
}


#[test]
fn test_parse_header() {
    use header::{self, q};
    let a: AcceptCharset = header::Header::parse_header(
        [b"iso-8859-5, iso-8859-6;q=0.8".to_vec()].as_ref()).unwrap();
    let b = AcceptCharset(vec![
        QualityItem { item: Charset::Iso_8859_5, quality: q(1.0) },
        QualityItem { item: Charset::Iso_8859_6, quality: q(0.8) },
    ]);
    assert_eq!(format!("{}", a), format!("{}", b));
    assert_eq!(a, b);
}
