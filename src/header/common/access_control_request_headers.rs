use unicase::Ascii;

header! {
    /// `Access-Control-Request-Headers` header, part of
    /// [CORS](http://www.w3.org/TR/cors/#access-control-request-headers-request-header)
    ///
    /// The `Access-Control-Request-Headers` header indicates which headers will
    /// be used in the actual request as part of the preflight request.
    /// during the actual request.
    ///
    /// # ABNF
    ///
    /// ```text
    /// Access-Control-Allow-Headers: "Access-Control-Allow-Headers" ":" #field-name
    /// ```
    ///
    /// # Example values
    /// * `accept-language, date`
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate hyper;
    /// # extern crate unicase;
    /// # fn main() {
    /// // extern crate unicase;
    ///
    /// use hyper::header::{Headers, AccessControlRequestHeaders};
    /// use unicase::Ascii;
    ///
    /// let mut headers = Headers::new();
    /// headers.set(
    ///     AccessControlRequestHeaders(vec![Ascii::new("date".to_owned())])
    /// );
    /// # }
    /// ```
    ///
    /// ```
    /// # extern crate hyper;
    /// # extern crate unicase;
    /// # fn main() {
    /// // extern crate unicase;
    ///
    /// use hyper::header::{Headers, AccessControlRequestHeaders};
    /// use unicase::Ascii;
    ///
    /// let mut headers = Headers::new();
    /// headers.set(
    ///     AccessControlRequestHeaders(vec![
    ///         Ascii::new("accept-language".to_owned()),
    ///         Ascii::new("date".to_owned()),
    ///     ])
    /// );
    /// # }
    /// ```
    (AccessControlRequestHeaders, "Access-Control-Request-Headers") => (Ascii<String>)*

    test_access_control_request_headers {
        test_header!(test1, vec![b"accept-language, date"]);
    }
}
