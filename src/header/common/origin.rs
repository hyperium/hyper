header! {
    /// 'Origin' request header,
    /// part of [CORS](http://www.w3.org/TR/cors/#origin-request-header)
    ///
    /// The 'Origin' header indicates where the cross-origin request
    /// or preflight request originates from.
    ///
    /// # ABNF
    /// ```plain
    /// Origin = url
    /// ```
    ///
    /// # Example value
    /// * `http://google.com`
    ///
    /// # Example
    /// ```
    /// use hyper::header::{Headers, Origin};
    ///
    /// let mut headers = Headers::new();
    /// headers.set(Origin("http://www.example.com".to_owned()));
    /// ```

    (Origin, "Origin") => [String]

    test_orgin {
        test_header!(test1, vec![b"http://google.com/"]);
    }
}
