use method::Method;

header! {
    /// `Access-Control-Allow-Methods` header, part of
    /// [CORS](http://www.w3.org/TR/cors/#access-control-allow-methods-response-header)
    ///
    /// The `Access-Control-Allow-Methods` header indicates, as part of the
    /// response to a preflight request, which methods can be used during the
    /// actual request.
    ///
    /// # ABNF
    /// ```plain
    /// Wildcard: "*"
    /// Access-Control-Allow-Methods: "Access-Control-Allow-Methods" ":" #Method / Wildcard
    /// ```
    ///
    /// # Example values
    /// * `PUT, DELETE, XMODIFY`
    ///
    /// # Examples
    /// ```
    /// use hyper::header::{Headers, AccessControlAllowMethods};
    /// use hyper::Method;
    ///
    /// let mut headers = Headers::new();
    /// headers.set(
    ///     AccessControlAllowMethods::Items(vec![Method::Get])
    /// );
    /// ```
    /// ```
    /// use hyper::header::{Headers, AccessControlAllowMethods};
    /// use hyper::Method;
    ///
    /// let mut headers = Headers::new();
    /// headers.set(
    ///     AccessControlAllowMethods::Items(vec![
    ///         Method::Get,
    ///         Method::Post,
    ///         Method::Patch,
    ///         Method::Extension("COPY".to_owned()),
    ///     ])
    /// );
    /// ```
    /// ```
    /// use hyper::header::{Headers, AccessControlAllowMethods};
    /// use hyper::Method;
    ///
    /// let mut headers = Headers::new();
    /// headers.set(
    ///     AccessControlAllowMethods::Any
    /// );
    /// ```
    (AccessControlAllowMethods, "Access-Control-Allow-Methods") => {Any / (Method)+}

    test_access_control_allow_methods {
        test_header!(test1, vec![b"GET"], Some(HeaderField::Items(vec![Method::Get])));
        test_header!(
            test2,
            vec![b"PUT, DELETE, XMODIFY"],
            Some(HeaderField::Items(
                vec![Method::Put,
                     Method::Delete,
                     Method::Extension("XMODIFY".to_owned())])));
        test_header!(test3, vec![b"*"], Some(HeaderField::Any));
    }

}
