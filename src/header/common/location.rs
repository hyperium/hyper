/// The `Location` header.
///
/// The Location response-header field is used to redirect the recipient to
/// a location other than the Request-URI for completion of the request or identification
/// of a new resource. For 201 (Created) responses, the Location is that of the new
/// resource which was created by the request. For 3xx responses, the location SHOULD
/// indicate the server's preferred URI for automatic redirection to the resource.
/// The field value consists of a single absolute URI.
///
/// Currently is just a String, but it should probably become a better type,
/// like url::Url or something.
#[derive(Clone, PartialEq, Debug)]
pub struct Location(pub String);

impl_header!(Location,
             "Location",
             String);

bench_header!(bench, Location, { vec![b"http://foo.com/hello:3000".to_vec()] });
