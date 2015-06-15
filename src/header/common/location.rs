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
    #[doc=""]
    #[doc="# Example values"]
    #[doc="* `/People.html#tim`"]
    #[doc="* `http://www.example.net/index.html`"]
    #[doc=""]
    #[doc="# Examples"]
    #[doc="```"]
    #[doc="use hyper::header::{Headers, Location};"]
    #[doc=""]
    #[doc="let mut headers = Headers::new();"]
    #[doc="headers.set(Location(\"/People.html#tim\".to_owned()));"]
    #[doc="```"]
    #[doc="```"]
    #[doc="use hyper::header::{Headers, Location};"]
    #[doc=""]
    #[doc="let mut headers = Headers::new();"]
    #[doc="headers.set(Location(\"http://www.example.com/index.html\".to_owned()));"]
    #[doc="```"]
    // TODO: Use URL
    (Location, "Location") => [String]

    test_location {
        // Testcase from RFC
        test_header!(test1, vec![b"/People.html#tim"]);
        test_header!(test2, vec![b"http://www.example.net/index.html"]);
    }

}

bench_header!(bench, Location, { vec![b"http://foo.com/hello:3000".to_vec()] });
