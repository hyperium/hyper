use header::EntityTag;

header! {
    #[doc="`If-Match` header, defined in"]
    #[doc="[RFC7232](https://tools.ietf.org/html/rfc7232#section-3.1)"]
    #[doc=""]
    #[doc="The `If-Match` header field makes the request method conditional on"]
    #[doc="the recipient origin server either having at least one current"]
    #[doc="representation of the target resource, when the field-value is \"*\","]
    #[doc="or having a current representation of the target resource that has an"]
    #[doc="entity-tag matching a member of the list of entity-tags provided in"]
    #[doc="the field-value."]
    #[doc=""]
    #[doc="An origin server MUST use the strong comparison function when"]
    #[doc="comparing entity-tags for `If-Match`, since the client"]
    #[doc="intends this precondition to prevent the method from being applied if"]
    #[doc="there have been any changes to the representation data."]
    #[doc=""]
    #[doc="# ABNF"]
    #[doc="```plain"]
    #[doc="If-Match = \"*\" / 1#entity-tag"]
    #[doc="```"]
    #[doc=""]
    #[doc="# Example values"]
    #[doc="* `\"xyzzy\"`"]
    #[doc="* \"xyzzy\", \"r2d2xxxx\", \"c3piozzzz\""]
    #[doc=""]
    #[doc="# Examples"]
    #[doc="```"]
    #[doc="use hyper::header::{Headers, IfMatch};"]
    #[doc=""]
    #[doc="let mut headers = Headers::new();"]
    #[doc="headers.set(IfMatch::Any);"]
    #[doc="```"]
    #[doc="```"]
    #[doc="use hyper::header::{Headers, IfMatch, EntityTag};"]
    #[doc=""]
    #[doc="let mut headers = Headers::new();"]
    #[doc="headers.set("]
    #[doc="    IfMatch::Items(vec!["]
    #[doc="        EntityTag::new(false, \"xyzzy\".to_owned()),"]
    #[doc="        EntityTag::new(false, \"foobar\".to_owned()),"]
    #[doc="        EntityTag::new(false, \"bazquux\".to_owned()),"]
    #[doc="    ])"]
    #[doc=");"]
    #[doc="```"]
    (IfMatch, "If-Match") => {Any / (EntityTag)+}

    test_if_match {
        test_header!(
            test1,
            vec![b"\"xyzzy\""],
            Some(HeaderField::Items(
                vec![EntityTag::new(false, "xyzzy".to_owned())])));
        test_header!(
            test2,
            vec![b"\"xyzzy\", \"r2d2xxxx\", \"c3piozzzz\""],
            Some(HeaderField::Items(
                vec![EntityTag::new(false, "xyzzy".to_owned()),
                     EntityTag::new(false, "r2d2xxxx".to_owned()),
                     EntityTag::new(false, "c3piozzzz".to_owned())])));
        test_header!(test3, vec![b"*"], Some(IfMatch::Any));
    }
}

bench_header!(star, IfMatch, { vec![b"*".to_vec()] });
bench_header!(single , IfMatch, { vec![b"\"xyzzy\"".to_vec()] });
bench_header!(multi, IfMatch,
              { vec![b"\"xyzzy\", \"r2d2xxxx\", \"c3piozzzz\"".to_vec()] });
