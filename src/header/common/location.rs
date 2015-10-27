header! {
    /// `Location` header, defined in
    /// [RFC7231](http://tools.ietf.org/html/rfc7231#section-7.1.2)
    ///
    /// The `Location` header field is used in some responses to refer to a
    /// specific resource in relation to the response.  The type of
    /// relationship is defined by the combination of request method and
    /// status code semantics.
    ///
    /// # ABNF
    /// ```plain
    /// Location = URI-reference
    /// ```
    ///
    /// # Example values
    /// * `/People.html#tim`
    /// * `http://www.example.net/index.html`
    ///
    /// # Examples
    /// ```
    /// use hyper::header::{Headers, Location};
    ///
    /// let mut headers = Headers::new();
    /// headers.set(Location("/People.html#tim".to_owned()));
    /// ```
    /// ```
    /// use hyper::header::{Headers, Location};
    ///
    /// let mut headers = Headers::new();
    /// headers.set(Location("http://www.example.com/index.html".to_owned()));
    /// ```
    // TODO: Use URL
    (Location, "Location") => [String]

    test_location {
        // Testcase from RFC
        test_header!(test1, vec![b"/People.html#tim"]);
        test_header!(test2, vec![b"http://www.example.net/index.html"]);
    }

}

bench_header!(bench, Location, { vec![b"http://foo.com/hello:3000".to_vec()] });
