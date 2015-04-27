use header::{Language, QualityItem};

header! {
    #[doc="`Content-Language` header, defined in"]
    #[doc="[RFC7231](https://tools.ietf.org/html/rfc7231#section-3.1.3.2)"]
    #[doc=""]
    #[doc="The `Content-Language` header field describes the natural language(s)"]
    #[doc="of the intended audience for the representation.  Note that this"]
    #[doc="might not be equivalent to all the languages used within the"]
    #[doc="representation."]
    #[doc=""]
    #[doc="# ABNF"]
    #[doc="```plain"]
    #[doc="Content-Language = 1#language-tag"]
    #[doc="```"]
    (ContentLanguage, "Content-Language") => (QualityItem<Language>)+

    test_content_language {
        test_header!(test1, vec![b"da"]);
        test_header!(test2, vec![b"mi, en"]);
    }
}
