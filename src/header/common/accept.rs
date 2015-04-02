use mime::Mime;

use header::QualityItem;

header! {
    #[doc="`Accept` header, defined in [RFC7231](http://tools.ietf.org/html/rfc7231#section-5.3.2)"]
    #[doc=""]
    #[doc="The `Accept` header field can be used by user agents to specify"]
    #[doc="response media types that are acceptable.  Accept header fields can"]
    #[doc="be used to indicate that the request is specifically limited to a"]
    #[doc="small set of desired types, as in the case of a request for an"]
    #[doc="in-line image"]
    #[doc=""]
    #[doc="# ABNF"]
    #[doc="```plain"]
    #[doc="Accept = #( media-range [ accept-params ] )"]
    #[doc=""]
    #[doc="media-range    = ( \"*/*\""]
    #[doc="                 / ( type \"/\" \"*\" )"]
    #[doc="                 / ( type \"/\" subtype )"]
    #[doc="                 ) *( OWS \";\" OWS parameter )"]
    #[doc="accept-params  = weight *( accept-ext )"]
    #[doc="accept-ext = OWS \";\" OWS token [ \"=\" ( token / quoted-string ) ]"]
    #[doc="```"]
    #[doc=""]
    #[doc="# Notes"]
    #[doc="* Using always Mime types to represent `media-range` differs from the ABNF."]
    #[doc="* **FIXME**: `accept-ext` is not supported."]
    (Accept, "Accept") => (QualityItem<Mime>)+
}

#[cfg(test)]
mod tests {
    use mime::*;

    use header::{Header, Quality, QualityItem, qitem};

    use super::Accept;

    #[test]
    fn test_parse_header_no_quality() {
        let a: Accept = Header::parse_header([b"text/plain; charset=utf-8".to_vec()].as_ref()).unwrap();
        let b = Accept(vec![
            qitem(Mime(TopLevel::Text, SubLevel::Plain, vec![(Attr::Charset, Value::Utf8)])),
        ]);
        assert_eq!(a, b);
    }

    #[test]
    fn test_parse_header_with_quality() {
        let a: Accept = Header::parse_header([b"text/plain; charset=utf-8; q=0.5".to_vec()].as_ref()).unwrap();
        let b = Accept(vec![
            QualityItem::new(Mime(TopLevel::Text, SubLevel::Plain, vec![(Attr::Charset, Value::Utf8)]), Quality(500)),
        ]);
        assert_eq!(a, b);
    }
}

bench_header!(bench, Accept, { vec![b"text/plain; q=0.5, text/html".to_vec()] });
