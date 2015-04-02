header! {
    #[doc="`Location` header, defined in"]
    #[doc="[RFC7231](http://tools.ietf.org/html/rfc7231#section-7.1.2)"]
    #[doc=""]
    #[doc="The `Location` header field is used in some responses to refer to a"]
    #[doc="specific resource in relation to the response.  The type of"]
    #[doc="relationship is defined by the combination of request method and"]
    #[doc="status code semantics."]
    #[doc=""]
    #[doc="# ABNF"]
    #[doc="```plain"]
    #[doc="Location = URI-reference"]
    #[doc="```"]
    // TODO: Use URL
    (Location, "Location") => [String]

}

bench_header!(bench, Location, { vec![b"http://foo.com/hello:3000".to_vec()] });
