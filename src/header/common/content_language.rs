use language_tags::LanguageTag;
use header::QualityItem;

header! {
    /// `Content-Language` header, defined in
    /// [RFC7231](https://tools.ietf.org/html/rfc7231#section-3.1.3.2)
    /// 
    /// The `Content-Language` header field describes the natural language(s)
    /// of the intended audience for the representation.  Note that this
    /// might not be equivalent to all the languages used within the
    /// representation.
    /// 
    /// # ABNF
    /// ```plain
    /// Content-Language = 1#language-tag
    /// ```
    /// 
    /// # Example values
    /// * `da`
    /// * `mi, en`
    /// 
    /// # Examples
    /// ```
    /// # extern crate hyper;
    /// # #[macro_use] extern crate language_tags;
    /// # use hyper::header::{Headers, ContentLanguage, qitem};
    /// # 
    /// # fn main() {
    /// let mut headers = Headers::new();
    /// headers.set(
    ///     ContentLanguage(vec![
    ///         qitem(langtag!(en)),
    ///     ])
    /// );
    /// # }
    /// ```
    /// ```
    /// # extern crate hyper;
    /// # #[macro_use] extern crate language_tags;
    /// # use hyper::header::{Headers, ContentLanguage, qitem};
    /// # 
    /// # fn main() {
    /// 
    /// let mut headers = Headers::new();
    /// headers.set(
    ///     ContentLanguage(vec![
    ///         qitem(langtag!(da)),
    ///         qitem(langtag!(en;;;GB)),
    ///     ])
    /// );
    /// # }
    /// ```
    (ContentLanguage, "Content-Language") => (QualityItem<LanguageTag>)+

    test_content_language {
        test_header!(test1, vec![b"da"]);
        test_header!(test2, vec![b"mi, en"]);
    }
}
