header! {
    /// `Server` header, defined in [RFC7231](http://tools.ietf.org/html/rfc7231#section-7.4.2)
    ///
    /// The `Server` header field contains information about the software
    /// used by the origin server to handle the request, which is often used
    /// by clients to help identify the scope of reported interoperability
    /// problems, to work around or tailor requests to avoid particular
    /// server limitations, and for analytics regarding server or operating
    /// system use.  An origin server MAY generate a Server field in its
    /// responses.
    ///
    /// # ABNF
    /// ```plain
    /// Server = product *( RWS ( product / comment ) )
    /// ```
    ///
    /// # Example values
    /// * `CERN/3.0 libwww/2.17`
    ///
    /// # Example
    /// ```
    /// use hyper::header::{Headers, Server};
    ///
    /// let mut headers = Headers::new();
    /// headers.set(Server("hyper/0.5.2".to_owned()));
    /// ```
    // TODO: Maybe parse as defined in the spec?
    (Server, "Server") => [String]

    test_server {
        // Testcase from RFC
        test_header!(test1, vec![b"CERN/3.0 libwww/2.17"]);
    }
}

bench_header!(bench, Server, { vec![b"Some String".to_vec()] });
