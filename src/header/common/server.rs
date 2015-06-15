header! {
    #[doc="`Server` header, defined in [RFC7231](http://tools.ietf.org/html/rfc7231#section-7.4.2)"]
    #[doc=""]
    #[doc="The `Server` header field contains information about the software"]
    #[doc="used by the origin server to handle the request, which is often used"]
    #[doc="by clients to help identify the scope of reported interoperability"]
    #[doc="problems, to work around or tailor requests to avoid particular"]
    #[doc="server limitations, and for analytics regarding server or operating"]
    #[doc="system use.  An origin server MAY generate a Server field in its"]
    #[doc="responses."]
    #[doc=""]
    #[doc="# ABNF"]
    #[doc="```plain"]
    #[doc="Server = product *( RWS ( product / comment ) )"]
    #[doc="```"]
    #[doc=""]
    #[doc="# Example values"]
    #[doc="* `CERN/3.0 libwww/2.17`"]
    #[doc=""]
    #[doc="# Example"]
    #[doc="```"]
    #[doc="use hyper::header::{Headers, Server};"]
    #[doc=""]
    #[doc="let mut headers = Headers::new();"]
    #[doc="headers.set(Server(\"hyper/0.5.2\".to_owned()));"]
    #[doc="```"]
    // TODO: Maybe parse as defined in the spec?
    (Server, "Server") => [String]

    test_server {
        // Testcase from RFC
        test_header!(test1, vec![b"CERN/3.0 libwww/2.17"]);
    }
}

bench_header!(bench, Server, { vec![b"Some String".to_vec()] });
