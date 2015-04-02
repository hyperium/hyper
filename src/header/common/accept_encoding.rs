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
}

#[cfg(test)]
mod tests {
    use header::{Encoding, Header, qitem, Quality, QualityItem};

    use super::*;

    #[test]
    fn test_parse_header() {
        let a: AcceptEncoding = Header::parse_header([b"gzip;q=1.0, identity; q=0.5".to_vec()].as_ref()).unwrap();
        let b = AcceptEncoding(vec![
            qitem(Encoding::Gzip),
            QualityItem::new(Encoding::Identity, Quality(500)),
        ]);
        assert_eq!(a, b);
    }
}
