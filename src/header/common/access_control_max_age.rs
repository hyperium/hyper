header! {
    /// `Access-Control-Max-Age` header, part of
    /// [CORS](http://www.w3.org/TR/cors/#access-control-max-age-response-header)
    ///
    /// The `Access-Control-Max-Age` header indicates how long the results of a
    /// preflight request can be cached in a preflight result cache.
    ///
    /// # ABNF
    /// ```plain
    /// Access-Control-Max-Age = \"Access-Control-Max-Age\" \":\" delta-seconds
    /// ```
    ///
    /// # Example values
    /// * `531`
    ///
    /// # Examples
    /// ```
    /// use hyper::header::{Headers, AccessControlMaxAge};
    ///
    /// let mut headers = Headers::new();
    /// headers.set(AccessControlMaxAge(1728000u32));
    /// ```
    (AccessControlMaxAge, "Access-Control-Max-Age") => [u32]

    test_access_control_max_age {
        test_header!(test1, vec![b"531"]);
    }
}
