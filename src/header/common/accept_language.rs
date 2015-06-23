use language_tags::LanguageTag;
use header::QualityItem;

header! {
    #[doc="`Accept-Language` header, defined in"]
    #[doc="[RFC7231](http://tools.ietf.org/html/rfc7231#section-5.3.5)"]
    #[doc=""]
    #[doc="The `Accept-Language` header field can be used by user agents to"]
    #[doc="indicate the set of natural languages that are preferred in the"]
    #[doc="response."]
    #[doc=""]
    #[doc="# ABNF"]
    #[doc="```plain"]
    #[doc="Accept-Language = 1#( language-range [ weight ] )"]
    #[doc="language-range  = <language-range, see [RFC4647], Section 2.1>"]
    #[doc="```"]
    #[doc=""]
    #[doc="# Example values"]
    #[doc="* `da, en-gb;q=0.8, en;q=0.7`"]
    #[doc="* `en-us;q=1.0, en;q=0.5, fr`"]
    #[doc=""]
    #[doc="# Examples"]
    #[doc="```"]
    #[doc="use hyper::LanguageTag;"]
    #[doc="use hyper::header::{Headers, AcceptLanguage, qitem};"]
    #[doc=""]
    #[doc="let mut headers = Headers::new();"]
    #[doc="let mut langtag: LanguageTag = Default::default();"]
    #[doc="langtag.language = Some(\"en\".to_owned());"]
    #[doc="langtag.region = Some(\"US\".to_owned());"]
    #[doc="headers.set("]
    #[doc="    AcceptLanguage(vec!["]
    #[doc="        qitem(langtag),"]
    #[doc="    ])"]
    #[doc=");"]
    #[doc="```"]
    #[doc="```"]
    #[doc="# extern crate hyper;"]
    #[doc="# #[macro_use] extern crate language_tags;"]
    #[doc="# use hyper::header::{Headers, AcceptLanguage, QualityItem, Quality, qitem};"]
    #[doc="# "]
    #[doc="# fn main() {"]
    #[doc="let mut headers = Headers::new();"]
    #[doc="headers.set("]
    #[doc="    AcceptLanguage(vec!["]
    #[doc="        qitem(langtag!(da)),"]
    #[doc="        QualityItem::new(langtag!(en;;;GB), Quality(800)),"]
    #[doc="        QualityItem::new(langtag!(en), Quality(700)),"]
    #[doc="    ])"]
    #[doc=");"]
    #[doc="# }"]
    #[doc="```"]
    (AcceptLanguage, "Accept-Language") => (QualityItem<LanguageTag>)+

    test_accept_language {
        // From the RFC
        test_header!(test1, vec![b"da, en-gb;q=0.8, en;q=0.7"]);
        // Own test
        test_header!(
            test2, vec![b"en-US, en; q=0.5, fr"],
            Some(AcceptLanguage(vec![
                qitem(langtag!(en;;;US)),
                QualityItem::new(langtag!(en), Quality(500)),
                qitem(langtag!(fr)),
        ])));
    }
}

bench_header!(bench, AcceptLanguage,
              { vec![b"en-us;q=1.0, en;q=0.5, fr".to_vec()] });
