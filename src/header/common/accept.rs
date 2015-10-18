use header::{MediaType, QualityItem};

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
    #[doc="# Example values"]
    #[doc="* `audio/*; q=0.2, audio/basic` (`*` value won't parse correctly)"]
    #[doc="* `text/plain; q=0.5, text/html, text/x-dvi; q=0.8, text/x-c`"]
    #[doc=""]
    #[doc="# Examples"]
    #[doc="```"]
    #[doc="use hyper::header::{Headers, Accept, qitem, MediaType};"]
    #[doc=""]
    #[doc="let mut headers = Headers::new();"]
    #[doc=""]
    #[doc="headers.set("]
    #[doc="    Accept(vec!["]
    #[doc="        qitem(MediaType::new(Some(\"text\"), None, Some(\"html\"), None)),"]
    #[doc="    ])"]
    #[doc=");"]
    #[doc="```"]
    #[doc="```"]
    #[doc="use hyper::header::{Headers, Accept, qitem, MediaType};"]
    #[doc=""]
    #[doc="let mut headers = Headers::new();"]
    #[doc="let mut mime = MediaType::new(Some(\"application\"), None, Some(\"json\"), None);"]
    #[doc="mime.set_charset_utf8();"]
    #[doc="headers.set("]
    #[doc="    Accept(vec!["]
    #[doc="        qitem(mime),"]
    #[doc="    ])"]
    #[doc=");"]
    #[doc="```"]
    #[doc="```"]
    #[doc="use hyper::header::{Headers, Accept, QualityItem, Quality, qitem, MediaType};"]
    #[doc=""]
    #[doc="let mut headers = Headers::new();"]
    #[doc=""]
    #[doc="headers.set("]
    #[doc="    Accept(vec!["]
    #[doc="        qitem(MediaType::new(Some(\"text\"), None, Some(\"html\"), None)),"]
    #[doc="        qitem(MediaType::new(Some(\"application\"), None, Some(\"xhtml\"), Some(\"xml\"))),"]
    #[doc="        QualityItem::new(MediaType::new(Some(\"text\"), None, Some(\"xml\"), None),"]
    #[doc="                         Quality(900)),"]
    #[doc="                         qitem(MediaType::new(Some(\"image\"), None, Some(\"webp\"), None)),"]
    #[doc="                         QualityItem::new(MediaType::new(None, None, None, None),"]
    #[doc="                                          Quality(800))"]
    #[doc="    ])"]
    #[doc=");"]
    #[doc="```"]
    #[doc=""]
    #[doc="# Notes"]
    #[doc="* Using always Mime types to represent `media-range` differs from the ABNF."]
    #[doc="* **FIXME**: `accept-ext` is not supported."]
    (Accept, "Accept") => (QualityItem<MediaType>)+

    test_accept {
        // Tests from the RFC
         test_header!(
            test1,
            vec![b"audio/*; q=0.2, audio/basic"],
            Some(HeaderField(vec![
                QualityItem::new(MediaType::new(Some("audio"), None, None, None), Quality(200)),
                qitem(MediaType::new(Some("audio"), None, Some("basic"), None)),
                ])));
        test_header!(
            test2,
            vec![b"text/plain; q=0.5, text/html, text/x-dvi; q=0.8, text/x-c"],
            Some(HeaderField(vec![
                QualityItem::new(
                    MediaType::new(Some("text"), None, Some("plain"), None),
                    Quality(500)),
                qitem(MediaType::new(Some("text"), None, Some("html"), None)),
                QualityItem::new(
                    MediaType::new(Some("text"), None, Some("x-dvi"), None),
                    Quality(800)),
                qitem(MediaType::new(Some("text"), None, Some("x-c"), None)),
                ])));
    }
}

bench_header!(bench, Accept, { vec![b"text/plain; q=0.5, text/html".to_vec()] });
