use header::Header;
use std::fmt::{mod, Show};
use super::from_one_raw_str;

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
#[deriving(Clone, PartialEq, Show)]
pub struct Location(pub String);

impl Header for Location {
    fn header_name(_: Option<Location>) -> &'static str {
        "Location"
    }

    fn parse_header(raw: &[Vec<u8>]) -> Option<Location> {
        from_one_raw_str(raw).map(|s| Location(s))
    }

    fn fmt_header(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let Location(ref value) = *self;
        value.fmt(fmt)
    }
}

