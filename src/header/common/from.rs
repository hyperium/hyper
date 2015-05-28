header! {
    #[doc="`From` header, defined in [RFC7231](http://tools.ietf.org/html/rfc7231#section-5.5.1)"]
    #[doc=""]
    #[doc="The `From` header field contains an Internet email address for a"]
    #[doc="human user who controls the requesting user agent.  The address ought"]
    #[doc="to be machine-usable."]
    #[doc="# ABNF"]
    #[doc="```plain"]
    #[doc="From    = mailbox"]
    #[doc="mailbox = <mailbox, see [RFC5322], Section 3.4>"]
    #[doc="```"]
    #[doc=""]
    #[doc="# Example"]
    #[doc="```"]
    #[doc="use hyper::header::{Headers, From};"]
    #[doc=""]
    #[doc="let mut headers = Headers::new();"]
    #[doc="headers.set(From(\"webmaster@example.org\".to_owned()));"]
    #[doc="```"]
    // FIXME: Maybe use mailbox?
    (From, "From") => [String]

    test_from {
        test_header!(test1, vec![b"webmaster@example.org"]);
    }
}
