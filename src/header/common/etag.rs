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
    #[doc=""]
    #[doc="# Example values"]
    #[doc="* `\"xyzzy\"`"]
    #[doc="* `W/\"xyzzy\"`"]
    #[doc="* `\"\"`"]
    #[doc=""]
    #[doc="# Examples"]
    #[doc="```"]
    #[doc="use hyper::header::{Headers, ETag, EntityTag};"]
    #[doc=""]
    #[doc="let mut headers = Headers::new();"]
    #[doc="headers.set(ETag(EntityTag::new(false, \"xyzzy\".to_owned())));"]
    #[doc="```"]
    #[doc="```"]
    #[doc="use hyper::header::{Headers, ETag, EntityTag};"]
    #[doc=""]
    #[doc="let mut headers = Headers::new();"]
    #[doc="headers.set(ETag(EntityTag::new(true, \"xyzzy\".to_owned())));"]
    #[doc="```"]
    (ETag, "ETag") => [EntityTag]

    test_etag {
        // From the RFC
        test_header!(test1,
            vec![b"\"xyzzy\""],
            Some(ETag(EntityTag::new(false, "xyzzy".to_owned()))));
        test_header!(test2,
            vec![b"W/\"xyzzy\""],
            Some(ETag(EntityTag::new(true, "xyzzy".to_owned()))));
        test_header!(test3,
            vec![b"\"\""],
            Some(ETag(EntityTag::new(false, "".to_owned()))));
        // Own tests
        test_header!(test4,
            vec![b"\"foobar\""],
            Some(ETag(EntityTag::new(false, "foobar".to_owned()))));
        test_header!(test5,
            vec![b"\"\""],
            Some(ETag(EntityTag::new(false, "".to_owned()))));
        test_header!(test6,
            vec![b"W/\"weak-etag\""],
            Some(ETag(EntityTag::new(true, "weak-etag".to_owned()))));
        test_header!(test7,
            vec![b"W/\"\x65\x62\""],
            Some(ETag(EntityTag::new(true, "\u{0065}\u{0062}".to_owned()))));
        test_header!(test8,
            vec![b"W/\"\""],
            Some(ETag(EntityTag::new(true, "".to_owned()))));
        test_header!(test9,
            vec![b"no-dquotes"],
            None::<ETag>);
        test_header!(test10,
            vec![b"w/\"the-first-w-is-case-sensitive\""],
            None::<ETag>);
        test_header!(test11,
            vec![b""],
            None::<ETag>);
        test_header!(test12,
            vec![b"\"unmatched-dquotes1"],
            None::<ETag>);
        test_header!(test13,
            vec![b"unmatched-dquotes2\""],
            None::<ETag>);
        test_header!(test14,
            vec![b"matched-\"dquotes\""],
            None::<ETag>);
    }
}

bench_header!(bench, ETag, { vec![b"W/\"nonemptytag\"".to_vec()] });
