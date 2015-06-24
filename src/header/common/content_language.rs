use language_tags::LanguageTag;
use header::QualityItem;

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
    #[doc=""]
    #[doc="# Example values"]
    #[doc="* `da`"]
    #[doc="* `mi, en`"]
    #[doc=""]
    #[doc="# Examples"]
    #[doc="```"]
    #[doc="# extern crate hyper;"]
    #[doc="# #[macro_use] extern crate language_tags;"]
    #[doc="# use hyper::header::{Headers, ContentLanguage, qitem};"]
    #[doc="# "]
    #[doc="# fn main() {"]
    #[doc="let mut headers = Headers::new();"]
    #[doc="headers.set("]
    #[doc="    ContentLanguage(vec!["]
    #[doc="        qitem(langtag!(en)),"]
    #[doc="    ])"]
    #[doc=");"]
    #[doc="# }"]
    #[doc="```"]
    #[doc="```"]
    #[doc="# extern crate hyper;"]
    #[doc="# #[macro_use] extern crate language_tags;"]
    #[doc="# use hyper::header::{Headers, ContentLanguage, qitem};"]
    #[doc="# "]
    #[doc="# fn main() {"]
    #[doc=""]
    #[doc="let mut headers = Headers::new();"]
    #[doc="headers.set("]
    #[doc="    ContentLanguage(vec!["]
    #[doc="        qitem(langtag!(da)),"]
    #[doc="        qitem(langtag!(en;;;GB)),"]
    #[doc="    ])"]
    #[doc=");"]
    #[doc="# }"]
    #[doc="```"]
    (ContentLanguage, "Content-Language") => (QualityItem<LanguageTag>)+

    test_content_language {
        test_header!(test1, vec![b"da"]);
        test_header!(test2, vec![b"mi, en"]);
    }
}
