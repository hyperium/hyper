use method::Method;

header! {
    /// `Allow` header, defined in [RFC7231](http://tools.ietf.org/html/rfc7231#section-7.4.1)
    ///
    /// The `Allow` header field lists the set of methods advertised as
    /// supported by the target resource.  The purpose of this field is
    /// strictly to inform the recipient of valid request methods associated
    /// with the resource.
    ///
    /// # ABNF
    /// ```plain
    /// Allow = #method
    /// ```
    ///
    /// # Example values
    /// * `GET, HEAD, PUT`
    /// * `OPTIONS, GET, PUT, POST, DELETE, HEAD, TRACE, CONNECT, PATCH, fOObAr`
    /// * ``
    ///
    /// # Examples
    /// ```
    /// use hyper::header::{Headers, Allow};
    /// use hyper::method::Method;
    ///
    /// let mut headers = Headers::new();
    /// headers.set(
    ///     Allow(vec![Method::Get])
    /// );
    /// ```
    /// ```
    /// use hyper::header::{Headers, Allow};
    /// use hyper::method::Method;
    ///
    /// let mut headers = Headers::new();
    /// headers.set(
    ///     Allow(vec![
    ///         Method::Get,
    ///         Method::Post,
    ///         Method::Patch,
    ///         Method::Extension("COPY".to_owned()),
    ///     ])
    /// );
    /// ```
    (Allow, "Allow") => (Method)*

    test_allow {
        // From the RFC
        test_header!(
            test1,
            vec![b"GET, HEAD, PUT"],
            Some(HeaderField(vec![Method::Get, Method::Head, Method::Put])));
        // Own tests
        test_header!(
            test2,
            vec![b"OPTIONS, GET, PUT, POST, DELETE, HEAD, TRACE, CONNECT, PATCH, fOObAr"],
            Some(HeaderField(vec![
                Method::Options,
                Method::Get,
                Method::Put,
                Method::Post,
                Method::Delete,
                Method::Head,
                Method::Trace,
                Method::Connect,
                Method::Patch,
                Method::Extension("fOObAr".to_owned())])));
        test_header!(
            test3,
            vec![b""],
            Some(HeaderField(Vec::<Method>::new())));
    }
}

bench_header!(bench,
    Allow, { vec![b"OPTIONS,GET,PUT,POST,DELETE,HEAD,TRACE,CONNECT,PATCH,fOObAr".to_vec()] });
