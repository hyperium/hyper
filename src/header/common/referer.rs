/// The `Referer` header.
///
/// The Referer header is used by user agents to inform server about
/// the page URL user has came from.
///
/// See alse [RFC 1945, section 10.13](http://tools.ietf.org/html/rfc1945#section-10.13).
///
/// Currently just a string, but maybe better replace it with url::Url or something like it.
#[derive(Clone, PartialEq, Debug)]
pub struct Referer(pub String);

impl_header!(Referer,
             "Referer",
             String);

bench_header!(bench, Referer, { vec![b"http://foo.com/hello:3000".to_vec()] });
