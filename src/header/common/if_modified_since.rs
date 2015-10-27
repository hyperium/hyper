use header::HttpDate;

header! {
    /// `If-Modified-Since` header, defined in
    /// [RFC7232](http://tools.ietf.org/html/rfc7232#section-3.3)
    /// 
    /// The `If-Modified-Since` header field makes a GET or HEAD request
    /// method conditional on the selected representation's modification date
    /// being more recent than the date provided in the field-value.
    /// Transfer of the selected representation's data is avoided if that
    /// data has not changed.
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
    /// use hyper::header::{Headers, IfModifiedSince, HttpDate};
    /// use time::{self, Duration};
    /// 
    /// let mut headers = Headers::new();
    /// headers.set(IfModifiedSince(HttpDate(time::now() - Duration::days(1))));
    /// # }
    /// ```
    (IfModifiedSince, "If-Modified-Since") => [HttpDate]

    test_if_modified_since {
        // Testcase from RFC
        test_header!(test1, vec![b"Sat, 29 Oct 1994 19:43:31 GMT"]);
    }
}

bench_header!(imf_fixdate, IfModifiedSince, { vec![b"Sun, 07 Nov 1994 08:48:37 GMT".to_vec()] });
bench_header!(rfc_850, IfModifiedSince, { vec![b"Sunday, 06-Nov-94 08:49:37 GMT".to_vec()] });
bench_header!(asctime, IfModifiedSince, { vec![b"Sun Nov  6 08:49:37 1994".to_vec()] });
