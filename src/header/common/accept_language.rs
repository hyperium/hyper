use language_tags::LanguageTag;
use header::QualityItem;

header! {
    /// `Accept-Language` header, defined in
    /// [RFC7231](http://tools.ietf.org/html/rfc7231#section-5.3.5)
    ///
    /// The `Accept-Language` header field can be used by user agents to
    /// indicate the set of natural languages that are preferred in the
    /// response.
    ///
    /// # ABNF
    /// ```plain
    /// Accept-Language = 1#( language-range [ weight ] )
    /// language-range  = <language-range, see [RFC4647], Section 2.1>
    /// ```
    ///
    /// # Example values
    /// * `da, en-gb;q=0.8, en;q=0.7`
    /// * `en-us;q=1.0, en;q=0.5, fr`
    ///
    /// # Examples
    /// ```
    /// use hyper::LanguageTag;
    /// use hyper::header::{Headers, AcceptLanguage, qitem};
    ///
    /// let mut headers = Headers::new();
    /// let mut langtag: LanguageTag = Default::default();
    /// langtag.language = Some("en".to_owned());
    /// langtag.region = Some("US".to_owned());
    /// headers.set(
    ///     AcceptLanguage(vec![
    ///         qitem(langtag),
    ///     ])
    /// );
    /// ```
    /// ```
    /// # extern crate hyper;
    /// # #[macro_use] extern crate language_tags;
    /// # use hyper::header::{Headers, AcceptLanguage, QualityItem, Quality, qitem};
    /// #
    /// # fn main() {
    /// let mut headers = Headers::new();
    /// headers.set(
    ///     AcceptLanguage(vec![
    ///         qitem(langtag!(da)),
    ///         QualityItem::new(langtag!(en;;;GB), Quality(800)),
    ///         QualityItem::new(langtag!(en), Quality(700)),
    ///     ])
    /// );
    /// # }
    /// ```
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
