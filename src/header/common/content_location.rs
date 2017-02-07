header! {
    /// `Content-Location` header, defined in
    /// [RFC7231](https://tools.ietf.org/html/rfc7231#section-3.1.4.2)
    ///
    /// The header can be used by both the client in requests and the server
    /// in responses with different semantics. Client sets `Content-Location`
    /// to refer to the URI where original representation of the body was
    /// obtained.
    ///
    /// In responses `Content-Location` represents URI for the representation
    /// that was content negotiated, created or for the response payload.
    ///
    /// # ABNF
    /// ```plain
    /// Content-Location = absolute-URI / partial-URI
    /// ```
    ///
    /// # Example values
    /// * `/hypertext/Overview.html`
    /// * `http://www.example.org/hypertext/Overview.html`
    ///
    /// # Examples
    ///
    /// ```
    /// use hyper::header::{Headers, ContentLocation};
    ///
    /// let mut headers = Headers::new();
    /// headers.set(ContentLocation("/hypertext/Overview.html".to_owned()));
    /// ```
    /// ```
    /// use hyper::header::{Headers, ContentLocation};
    ///
    /// let mut headers = Headers::new();
    /// headers.set(ContentLocation("http://www.example.org/hypertext/Overview.html".to_owned()));
    /// ```
    // TODO: use URL
    (ContentLocation, "Content-Location") => [String]

    test_content_location {
        test_header!(partial_query, vec![b"/hypertext/Overview.html?q=tim"]);

        test_header!(absolute, vec![b"http://www.example.org/hypertext/Overview.html"]);
    }
}
