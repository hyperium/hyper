use unicase::UniCase;

header! {
    /// `Access-Control-Allow-Headers` header, part of
    /// [CORS](http://www.w3.org/TR/cors/#access-control-allow-headers-response-header)
    ///
    /// The `Access-Control-Allow-Headers` header indicates, as part of the
    /// response to a preflight request, which header field names can be used
    /// during the actual request.
    ///
    /// # ABNF
    /// ```plain
    /// Access-Control-Allow-Headers: "Access-Control-Allow-Headers" ":" #field-name
    /// ```
    ///
    /// # Example values
    /// * `accept-language, date`
    ///
    /// # Examples
    /// ```
    /// # extern crate hyper;
    /// # extern crate unicase;
    /// # fn main() {
    /// // extern crate unicase;
    ///
    /// use hyper::header::{Headers, AccessControlAllowHeaders};
    /// use unicase::UniCase;
    ///
    /// let mut headers = Headers::new();
    /// headers.set(
    ///     AccessControlAllowHeaders(vec![UniCase("date".to_owned())])
    /// );
    /// # }
    /// ```
    /// ```
    /// # extern crate hyper;
    /// # extern crate unicase;
    /// # fn main() {
    /// // extern crate unicase;
    ///
    /// use hyper::header::{Headers, AccessControlAllowHeaders};
    /// use unicase::UniCase;
    ///
    /// let mut headers = Headers::new();
    /// headers.set(
    ///     AccessControlAllowHeaders(vec![
    ///         UniCase("accept-language".to_owned()),
    ///         UniCase("date".to_owned()),
    ///     ])
    /// );
    /// # }
    /// ```
    (AccessControlAllowHeaders, "Access-Control-Allow-Headers") => (UniCase<String>)*

    test_access_control_allow_headers {
        test_header!(test1, vec![b"accept-language, date"]);
    }
}
