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
    (IfMatch, "If-Match") => {Any / (EntityTag)+}
}

#[test]
fn test_parse_header() {
    use header::Header;
    {
        let a: IfMatch = Header::parse_header(
        [b"*".to_vec()].as_ref()).unwrap();
        assert_eq!(a, IfMatch::Any);
    }
    {
        let a: IfMatch = Header::parse_header(
            [b"\"xyzzy\", \"r2d2xxxx\", \"c3piozzzz\"".to_vec()].as_ref()).unwrap();
        let b = IfMatch::Items(
            vec![EntityTag::new(false, "xyzzy".to_string()),
                 EntityTag::new(false, "r2d2xxxx".to_string()),
                 EntityTag::new(false, "c3piozzzz".to_string())]);
        assert_eq!(a, b);
    }
}

bench_header!(star, IfMatch, { vec![b"*".to_vec()] });
bench_header!(single , IfMatch, { vec![b"\"xyzzy\"".to_vec()] });
bench_header!(multi, IfMatch,
              { vec![b"\"xyzzy\", \"r2d2xxxx\", \"c3piozzzz\"".to_vec()] });
