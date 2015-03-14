use mime::Mime;

/// The `Content-Type` header.
///
/// Used to describe the MIME type of message body. Can be used with both
/// requests and responses.
#[derive(Clone, PartialEq, Debug)]
pub struct ContentType(pub Mime);

impl_header!(ContentType,
             "Content-Type",
             Mime);

bench_header!(bench, ContentType, { vec![b"application/json; charset=utf-8".to_vec()] });
