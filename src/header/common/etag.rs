use header::EntityTag;

header! {
    #[doc="`ETag` header, defined in [RFC7232](http://tools.ietf.org/html/rfc7232#section-2.3)"]
    #[doc=""]
    #[doc="The `ETag` header field in a response provides the current entity-tag"]
    #[doc="for the selected representation, as determined at the conclusion of"]
    #[doc="handling the request.  An entity-tag is an opaque validator for"]
    #[doc="differentiating between multiple representations of the same"]
    #[doc="resource, regardless of whether those multiple representations are"]
    #[doc="due to resource state changes over time, content negotiation"]
    #[doc="resulting in multiple representations being valid at the same time,"]
    #[doc="or both.  An entity-tag consists of an opaque quoted string, possibly"]
    #[doc="prefixed by a weakness indicator."]
    #[doc=""]
    #[doc="# ABNF"]
    #[doc="```plain"]
    #[doc="ETag       = entity-tag"]
    #[doc="```"]
    (ETag, "ETag") => [EntityTag]
}

#[cfg(test)]
mod tests {
    use super::ETag;
    use header::{Header,EntityTag};

    #[test]
    fn test_etag_successes() {
        // Expected successes
        let mut etag: Option<ETag>;

        etag = Header::parse_header([b"\"foobar\"".to_vec()].as_ref());
        assert_eq!(etag, Some(ETag(EntityTag::new(false, "foobar".to_string()))));

        etag = Header::parse_header([b"\"\"".to_vec()].as_ref());
        assert_eq!(etag, Some(ETag(EntityTag::new(false, "".to_string()))));

        etag = Header::parse_header([b"W/\"weak-etag\"".to_vec()].as_ref());
        assert_eq!(etag, Some(ETag(EntityTag::new(true, "weak-etag".to_string()))));

        etag = Header::parse_header([b"W/\"\x65\x62\"".to_vec()].as_ref());
        assert_eq!(etag, Some(ETag(EntityTag::new(true, "\u{0065}\u{0062}".to_string()))));

        etag = Header::parse_header([b"W/\"\"".to_vec()].as_ref());
        assert_eq!(etag, Some(ETag(EntityTag::new(true, "".to_string()))));
    }

    #[test]
    fn test_etag_failures() {
        // Expected failures
        let mut etag: Option<ETag>;

        etag = Header::parse_header([b"no-dquotes".to_vec()].as_ref());
        assert_eq!(etag, None);

        etag = Header::parse_header([b"w/\"the-first-w-is-case-sensitive\"".to_vec()].as_ref());
        assert_eq!(etag, None);

        etag = Header::parse_header([b"".to_vec()].as_ref());
        assert_eq!(etag, None);

        etag = Header::parse_header([b"\"unmatched-dquotes1".to_vec()].as_ref());
        assert_eq!(etag, None);

        etag = Header::parse_header([b"unmatched-dquotes2\"".to_vec()].as_ref());
        assert_eq!(etag, None);

        etag = Header::parse_header([b"matched-\"dquotes\"".to_vec()].as_ref());
        assert_eq!(etag, None);
    }
}

bench_header!(bench, ETag, { vec![b"W/\"nonemptytag\"".to_vec()] });
