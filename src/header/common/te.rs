use header::{Encoding, QualityItem};

header! {
    /// `TE` header, defined in
    /// [RFC7230](http://tools.ietf.org/html/rfc7230#section-4.3)
    ///
    /// As RFC7230 states, "The "TE" header field in a request indicates what transfer codings,
    /// besides chunked, the client is willing to accept in response, and
    /// whether or not the client is willing to accept trailer fields in a
    /// chunked transfer coding."
    ///
    /// For HTTP/1.1 compliant clients `chunked` transfer codings are assumed to be acceptable and
    /// so should never appear in this header.
    ///
    /// # ABNF
    /// ```plain
    /// TE        = "TE" ":" #( t-codings )
    /// t-codings = "trailers" | ( transfer-extension [ accept-params ] )
    /// ```
    ///
    /// # Example values
    /// * `trailers`
    /// * `trailers, deflate;q=0.5`
    /// * ``
    ///
    /// # Examples
    /// ```
    /// use hyper::header::{Headers, Te, Encoding, qitem};
    ///
    /// let mut headers = Headers::new();
    /// headers.set(
    ///     Te(vec![qitem(Encoding::Trailers)])
    /// );
    /// ```
    /// ```
    /// use hyper::header::{Headers, Te, Encoding, qitem};
    ///
    /// let mut headers = Headers::new();
    /// headers.set(
    ///     Te(vec![
    ///         qitem(Encoding::Trailers),
    ///         qitem(Encoding::Gzip),
    ///         qitem(Encoding::Deflate),
    ///     ])
    /// );
    /// ```
    /// ```
    /// use hyper::header::{Headers, Te, Encoding, QualityItem, q, qitem};
    ///
    /// let mut headers = Headers::new();
    /// headers.set(
    ///     Te(vec![
    ///         qitem(Encoding::Trailers),
    ///         QualityItem::new(Encoding::Gzip, q(600)),
    ///         QualityItem::new(Encoding::EncodingExt("*".to_owned()), q(0)),
    ///     ])
    /// );
    /// ```
    (Te, "TE") => (QualityItem<Encoding>)*

    test_te {
        // From the RFC
        test_header!(test1, vec![b"trailers"]);
        test_header!(test2, vec![b"trailers, deflate;q=0.5"]);
        test_header!(test3, vec![b""]);
    }
}
