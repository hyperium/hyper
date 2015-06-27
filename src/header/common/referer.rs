header! {
    #[doc="`Referer` header, defined in"]
    #[doc="[RFC7231](http://tools.ietf.org/html/rfc7231#section-5.5.2)"]
    #[doc=""]
    #[doc="The `Referer` [sic] header field allows the user agent to specify a"]
    #[doc="URI reference for the resource from which the target URI was obtained"]
    #[doc="(i.e., the \"referrer\", though the field name is misspelled).  A user"]
    #[doc="agent MUST NOT include the fragment and userinfo components of the"]
    #[doc="URI reference, if any, when generating the Referer field value."]
    #[doc=""]
    #[doc="# ABNF"]
    #[doc="```plain"]
    #[doc="Referer = absolute-URI / partial-URI"]
    #[doc="```"]
    #[doc=""]
    #[doc="# Example values"]
    #[doc="* `http://www.example.org/hypertext/Overview.html`"]
    #[doc=""]
    #[doc="# Examples"]
    #[doc="```"]
    #[doc="use hyper::header::{Headers, Referer};"]
    #[doc=""]
    #[doc="let mut headers = Headers::new();"]
    #[doc="headers.set(Referer(\"/People.html#tim\".to_owned()));"]
    #[doc="```"]
    #[doc="```"]
    #[doc="use hyper::header::{Headers, Referer};"]
    #[doc=""]
    #[doc="let mut headers = Headers::new();"]
    #[doc="headers.set(Referer(\"http://www.example.com/index.html\".to_owned()));"]
    #[doc="```"]
    // TODO Use URL
    (Referer, "Referer") => [String]

    test_referer {
        // Testcase from the RFC
        test_header!(test1, vec![b"http://www.example.org/hypertext/Overview.html"]);
    }
}

bench_header!(bench, Referer, { vec![b"http://foo.com/hello:3000".to_vec()] });
