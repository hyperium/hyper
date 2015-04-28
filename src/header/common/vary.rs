use unicase::UniCase;

header! {
    #[doc="`Vary` header, defined in [RFC7231](https://tools.ietf.org/html/rfc7231#section-7.1.4)"]
    #[doc=""]
    #[doc="The \"Vary\" header field in a response describes what parts of a"]
    #[doc="request message, aside from the method, Host header field, and"]
    #[doc="request target, might influence the origin server's process for"]
    #[doc="selecting and representing this response.  The value consists of"]
    #[doc="either a single asterisk (\"*\") or a list of header field names"]
    #[doc="(case-insensitive)."]
    #[doc=""]
    #[doc="# ABNF"]
    #[doc="```plain"]
    #[doc="Vary = \"*\" / 1#field-name"]
    #[doc="```"]
    #[doc=""]
    #[doc="# Example values"]
    #[doc="* `accept-encoding, accept-language`"]
    (Vary, "Vary") => {Any / (UniCase<String>)+}

    test_vary {
        test_header!(test1, vec![b"accept-encoding, accept-language"]);

        #[test]
        fn test2() {
            let mut vary: Option<Vary>;

            vary = Header::parse_header([b"*".to_vec()].as_ref());
            assert_eq!(vary, Some(Vary::Any));

            vary = Header::parse_header([b"etag,cookie,allow".to_vec()].as_ref());
            assert_eq!(vary, Some(Vary::Items(vec!["eTag".parse().unwrap(),
                                                    "cookIE".parse().unwrap(),
                                                    "AlLOw".parse().unwrap(),])));
        }
    }
}
