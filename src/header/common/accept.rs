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
    #[doc="# Example values"]
    #[doc="* `audio/*; q=0.2, audio/basic` (`*` value won't parse correctly)"]
    #[doc="* `text/plain; q=0.5, text/html, text/x-dvi; q=0.8, text/x-c`"]
    #[doc=""]
    #[doc="# Examples"]
    #[doc="```"]
    #[doc="use hyper::header::{Headers, Accept, qitem};"]
    #[doc="use hyper::mime::{Mime, TopLevel, SubLevel};"]
    #[doc=""]
    #[doc="let mut headers = Headers::new();"]
    #[doc=""]
    #[doc="headers.set("]
    #[doc="    Accept(vec!["]
    #[doc="        qitem(Mime(TopLevel::Text, SubLevel::Html, vec![])),"]
    #[doc="    ])"]
    #[doc=");"]
    #[doc="```"]
    #[doc="```"]
    #[doc="use hyper::header::{Headers, Accept, qitem};"]
    #[doc="use hyper::mime::{Mime, TopLevel, SubLevel, Attr, Value};"]
    #[doc=""]
    #[doc="let mut headers = Headers::new();"]
    #[doc="headers.set("]
    #[doc="    Accept(vec!["]
    #[doc="        qitem(Mime(TopLevel::Application, SubLevel::Json,"]
    #[doc="                   vec![(Attr::Charset, Value::Utf8)])),"]
    #[doc="    ])"]
    #[doc=");"]
    #[doc="```"]
    #[doc="```"]
    #[doc="use hyper::header::{Headers, Accept, QualityItem, Quality, qitem};"]
    #[doc="use hyper::mime::{Mime, TopLevel, SubLevel};"]
    #[doc=""]
    #[doc="let mut headers = Headers::new();"]
    #[doc=""]
    #[doc="headers.set("]
    #[doc="    Accept(vec!["]
    #[doc="        qitem(Mime(TopLevel::Text, SubLevel::Html, vec![])),"]
    #[doc="        qitem(Mime(TopLevel::Application,"]
    #[doc="                   SubLevel::Ext(\"xhtml+xml\".to_owned()), vec![])),"]
    #[doc="        QualityItem::new(Mime(TopLevel::Application, SubLevel::Xml, vec![]),"]
    #[doc="                         Quality(900)),"]
    #[doc="                         qitem(Mime(TopLevel::Image,"]
    #[doc="                                    SubLevel::Ext(\"webp\".to_owned()), vec![])),"]
    #[doc="                         QualityItem::new(Mime(TopLevel::Star, SubLevel::Star, vec![]),"]
    #[doc="                                          Quality(800))"]
    #[doc="    ])"]
    #[doc=");"]
    #[doc="```"]
    #[doc=""]
    #[doc="# Notes"]
    #[doc="* Using always Mime types to represent `media-range` differs from the ABNF."]
    #[doc="* **FIXME**: `accept-ext` is not supported."]
    (Accept, "Accept") => (QualityItem<Mime>)+

    test_accept {
        // Tests from the RFC
        // FIXME: Test fails, first value containing a "*" fails to parse
        // test_header!(
        //    test1,
        //    vec![b"audio/*; q=0.2, audio/basic"],
        //    Some(HeaderField(vec![
        //        QualityItem::new(Mime(TopLevel::Audio, SubLevel::Star, vec![]), Quality(200)),
        //        qitem(Mime(TopLevel::Audio, SubLevel::Ext("basic".to_owned()), vec![])),
        //        ])));
        test_header!(
            test2,
            vec![b"text/plain; q=0.5, text/html, text/x-dvi; q=0.8, text/x-c"],
            Some(HeaderField(vec![
                QualityItem::new(Mime(TopLevel::Text, SubLevel::Plain, vec![]), Quality(500)),
                qitem(Mime(TopLevel::Text, SubLevel::Html, vec![])),
                QualityItem::new(
                    Mime(TopLevel::Text, SubLevel::Ext("x-dvi".to_owned()), vec![]),
                    Quality(800)),
                qitem(Mime(TopLevel::Text, SubLevel::Ext("x-c".to_owned()), vec![])),
                ])));
        // Custom tests
        test_header!(
            test3,
            vec![b"text/plain; charset=utf-8"],
            Some(Accept(vec![
                qitem(Mime(TopLevel::Text, SubLevel::Plain, vec![(Attr::Charset, Value::Utf8)])),
                ])));
        test_header!(
            test4,
            vec![b"text/plain; charset=utf-8; q=0.5"],
            Some(Accept(vec![
                QualityItem::new(Mime(TopLevel::Text,
                    SubLevel::Plain, vec![(Attr::Charset, Value::Utf8)]),
                    Quality(500)),
            ])));
    }
}

bench_header!(bench, Accept, { vec![b"text/plain; q=0.5, text/html".to_vec()] });
