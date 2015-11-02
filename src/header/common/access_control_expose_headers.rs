use unicase::UniCase;

header! {
    /// `Access-Control-Expose-Headers` header, part of
    /// [CORS](http://www.w3.org/TR/cors/#access-control-expose-headers-response-header)
    ///
    /// The Access-Control-Expose-Headers header indicates which headers are safe to expose to the
    /// API of a CORS API specification.
    ///
    /// # ABNF
    /// ```plain
    /// Access-Control-Expose-Headers = "Access-Control-Expose-Headers" ":" #field-name
    /// ```
    ///
    /// # Example values
    /// * `ETag, Content-Length`
    ///
    /// # Examples
    /// ```
    /// # extern crate hyper;
    /// # extern crate unicase;
    /// # fn main() {
    /// // extern crate unicase;
    ///
    /// use hyper::header::{Headers, AccessControlExposeHeaders};
    /// use unicase::UniCase;
    ///
    /// let mut headers = Headers::new();
    /// headers.set(
    ///     AccessControlExposeHeaders(vec![
    ///         UniCase("etag".to_owned()),
    ///         UniCase("content-length".to_owned())
    ///     ])
    /// );
    /// # }
    /// ```
    /// ```
    /// # extern crate hyper;
    /// # extern crate unicase;
    /// # fn main() {
    /// // extern crate unicase;
    ///
    /// use hyper::header::{Headers, AccessControlExposeHeaders};
    /// use unicase::UniCase;
    ///
    /// let mut headers = Headers::new();
    /// headers.set(
    ///     AccessControlExposeHeaders(vec![
    ///         UniCase("etag".to_owned()),
    ///         UniCase("content-length".to_owned())
    ///     ])
    /// );
    /// # }
    /// ```
    (AccessControlExposeHeaders, "Access-Control-Expose-Headers") => (UniCase<String>)*

    test_access_control_expose_headers {
        test_header!(test1, vec![b"etag, content-length"]);
    }
}
