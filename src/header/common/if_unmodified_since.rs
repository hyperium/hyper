use header::HttpDate;

header! {
    /// `If-Unmodified-Since` header, defined in
    /// [RFC7232](http://tools.ietf.org/html/rfc7232#section-3.4)
    /// 
    /// The `If-Unmodified-Since` header field makes the request method
    /// conditional on the selected representation's last modification date
    /// being earlier than or equal to the date provided in the field-value.
    /// This field accomplishes the same purpose as If-Match for cases where
    /// the user agent does not have an entity-tag for the representation.
    /// 
    /// # ABNF
    /// ```plain
    /// If-Unmodified-Since = HTTP-date
    /// ```
    /// 
    /// # Example values
    /// * `Sat, 29 Oct 1994 19:43:31 GMT`
    /// 
    /// # Example
    /// ```
    /// # extern crate hyper;
    /// # extern crate time;
    /// # fn main() {
    /// // extern crate time;
    /// 
    /// use hyper::header::{Headers, IfUnmodifiedSince, HttpDate};
    /// use time::{self, Duration};
    /// 
    /// let mut headers = Headers::new();
    /// headers.set(IfUnmodifiedSince(HttpDate(time::now() - Duration::days(1))));
    /// # }
    /// ```
    (IfUnmodifiedSince, "If-Unmodified-Since") => [HttpDate]

    test_if_unmodified_since {
        // Testcase from RFC
        test_header!(test1, vec![b"Sat, 29 Oct 1994 19:43:31 GMT"]);
    }
}

bench_header!(imf_fixdate, IfUnmodifiedSince, { vec![b"Sun, 07 Nov 1994 08:48:37 GMT".to_vec()] });
bench_header!(rfc_850, IfUnmodifiedSince, { vec![b"Sunday, 06-Nov-94 08:49:37 GMT".to_vec()] });
bench_header!(asctime, IfUnmodifiedSince, { vec![b"Sun Nov  6 08:49:37 1994".to_vec()] });
