use header::HttpDate;

header! {
    /// `Date` header, defined in [RFC7231](http://tools.ietf.org/html/rfc7231#section-7.1.1.2)
    /// 
    /// The `Date` header field represents the date and time at which the
    /// message was originated.
    /// 
    /// # ABNF
    /// ```plain
    /// Date = HTTP-date
    /// ```
    /// 
    /// # Example values
    /// * `Tue, 15 Nov 1994 08:12:31 GMT`
    /// 
    /// # Example
    /// ```
    /// # extern crate time;
    /// # extern crate hyper;
    /// # fn main() {
    /// // extern crate time;
    /// 
    /// use hyper::header::{Headers, Date, HttpDate};
    /// use time;
    /// 
    /// let mut headers = Headers::new();
    /// headers.set(Date(HttpDate(time::now())));
    /// # }
    /// ```
    (Date, "Date") => [HttpDate]

    test_date {
        test_header!(test1, vec![b"Tue, 15 Nov 1994 08:12:31 GMT"]);
    }
}

bench_header!(imf_fixdate, Date, { vec![b"Sun, 07 Nov 1994 08:48:37 GMT".to_vec()] });
bench_header!(rfc_850, Date, { vec![b"Sunday, 06-Nov-94 08:49:37 GMT".to_vec()] });
bench_header!(asctime, Date, { vec![b"Sun Nov  6 08:49:37 1994".to_vec()] });
