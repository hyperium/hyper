use header::{Language, QualityItem};

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
    (AcceptLanguage, "Accept-Language") => (QualityItem<Language>)+

    test_accept_language {
        // From the RFC
        test_header!(test1, vec![b"da, en-gb;q=0.8, en;q=0.7"]);
        // Own test
        test_header!(
            test2, vec![b"en-us, en; q=0.5, fr"],
            Some(AcceptLanguage(vec![
                qitem(Language {primary: "en".to_string(), sub: Some("us".to_string())}),
                QualityItem::new(Language{primary: "en".to_string(), sub: None}, Quality(500)),
                qitem(Language {primary: "fr".to_string(), sub: None}),
        ])));
    }
}

bench_header!(bench, AcceptLanguage,
              { vec![b"en-us;q=1.0, en;q=0.5, fr".to_vec()] });
