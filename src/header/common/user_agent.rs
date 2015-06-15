header! {
    #[doc="`User-Agent` header, defined in"]
    #[doc="[RFC7231](http://tools.ietf.org/html/rfc7231#section-5.5.3)"]
    #[doc=""]
    #[doc="The `User-Agent` header field contains information about the user"]
    #[doc="agent originating the request, which is often used by servers to help"]
    #[doc="identify the scope of reported interoperability problems, to work"]
    #[doc="around or tailor responses to avoid particular user agent"]
    #[doc="limitations, and for analytics regarding browser or operating system"]
    #[doc="use.  A user agent SHOULD send a User-Agent field in each request"]
    #[doc="unless specifically configured not to do so."]
    #[doc=""]
    #[doc="# ABNF"]
    #[doc="```plain"]
    #[doc="User-Agent = product *( RWS ( product / comment ) )"]
    #[doc="product         = token [\"/\" product-version]"]
    #[doc="product-version = token"]
    #[doc="```"]
    #[doc=""]
    #[doc="# Example values"]
    #[doc="* `CERN-LineMode/2.15 libwww/2.17b3`"]
    #[doc="* `Bunnies`"]
    #[doc=""]
    #[doc="# Notes"]
    #[doc="* The parser does not split the value"]
    #[doc=""]
    #[doc="# Example"]
    #[doc="```"]
    #[doc="use hyper::header::{Headers, UserAgent};"]
    #[doc=""]
    #[doc="let mut headers = Headers::new();"]
    #[doc="headers.set(UserAgent(\"hyper/0.5.2\".to_owned()));"]
    #[doc="```"]
    (UserAgent, "User-Agent") => [String]

    test_user_agent {
        // Testcase from RFC
        test_header!(test1, vec![b"CERN-LineMode/2.15 libwww/2.17b3"]);
        // Own testcase
        test_header!(test2, vec![b"Bunnies"], Some(UserAgent("Bunnies".to_owned())));
    }
}
