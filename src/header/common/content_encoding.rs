use header::Encoding;

header! {
    /// `Content-Encoding` header, defined in
    /// [RFC7231](http://tools.ietf.org/html/rfc7231#section-3.1.2.2)
    /// 
    /// The `Content-Encoding` header field indicates what content codings
    /// have been applied to the representation, beyond those inherent in the
    /// media type, and thus what decoding mechanisms have to be applied in
    /// order to obtain data in the media type referenced by the Content-Type
    /// header field.  Content-Encoding is primarily used to allow a
    /// representation's data to be compressed without losing the identity of
    /// its underlying media type.
    /// 
    /// # ABNF
    /// ```plain
    /// Content-Encoding = 1#content-coding
    /// ```
    /// 
    /// # Example values
    /// * `gzip`
    /// 
    /// # Examples
    /// ```
    /// use hyper::header::{Headers, ContentEncoding, Encoding};
    /// 
    /// let mut headers = Headers::new();
    /// headers.set(ContentEncoding(Encoding::Chunked));
    /// ```
    /// ```
    /// use hyper::header::{Headers, ContentEncoding, Encoding};
    /// 
    /// let mut headers = Headers::new();
    /// headers.set(ContentEncoding(Encoding::Gzip));
    /// headers.set(ContentEncoding(Encoding::Chunked));
    /// ```
    (ContentEncoding, "Content-Encoding") => [Encoding]

    test_content_encoding {
        /// Testcase from the RFC
        test_header!(test1, vec![b"gzip"], Some(ContentEncoding(Encoding::Gzip)));
    }
}

bench_header!(single, ContentEncoding, { vec![b"gzip".to_vec()] });
//bench_header!(multiple, ContentEncoding, { vec![b"gzip, deflate".to_vec()] }); // not allowed
