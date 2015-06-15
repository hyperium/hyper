use header::Encoding;

header! {
    #[doc="`Transfer-Encoding` header, defined in"]
    #[doc="[RFC7230](http://tools.ietf.org/html/rfc7230#section-3.3.1)"]
    #[doc=""]
    #[doc="The `Transfer-Encoding` header field lists the transfer coding names"]
    #[doc="corresponding to the sequence of transfer codings that have been (or"]
    #[doc="will be) applied to the payload body in order to form the message"]
    #[doc="body."]
    #[doc=""]
    #[doc="# ABNF"]
    #[doc="```plain"]
    #[doc="Transfer-Encoding = 1#transfer-coding"]
    #[doc="```"]
    #[doc=""]
    #[doc="# Example values"]
    #[doc="* `gzip, chunked`"]
    #[doc=""]
    #[doc="# Example"]
    #[doc="```"]
    #[doc="use hyper::header::{Headers, TransferEncoding, Encoding};"]
    #[doc=""]
    #[doc="let mut headers = Headers::new();"]
    #[doc="headers.set("]
    #[doc="    TransferEncoding(vec!["]
    #[doc="        Encoding::Gzip,"]
    #[doc="        Encoding::Chunked,"]
    #[doc="    ])"]
    #[doc=");"]
    #[doc="```"]
    (TransferEncoding, "Transfer-Encoding") => (Encoding)+

    transfer_encoding {
        test_header!(
            test1,
            vec![b"gzip, chunked"],
            Some(HeaderField(
                vec![Encoding::Gzip, Encoding::Chunked]
                )));

    }
}

bench_header!(normal, TransferEncoding, { vec![b"chunked, gzip".to_vec()] });
bench_header!(ext, TransferEncoding, { vec![b"ext".to_vec()] });
