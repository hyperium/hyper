use std::fmt::{self, Display};
use header::{self, Header, Raw, EntityTag, HttpDate};

/// `If-Range` header, defined in [RFC7233](http://tools.ietf.org/html/rfc7233#section-3.2)
///
/// If a client has a partial copy of a representation and wishes to have
/// an up-to-date copy of the entire representation, it could use the
/// Range header field with a conditional GET (using either or both of
/// If-Unmodified-Since and If-Match.)  However, if the precondition
/// fails because the representation has been modified, the client would
/// then have to make a second request to obtain the entire current
/// representation.
///
/// The `If-Range` header field allows a client to \"short-circuit\" the
/// second request.  Informally, its meaning is as follows: if the
/// representation is unchanged, send me the part(s) that I am requesting
/// in Range; otherwise, send me the entire representation.
///
/// # ABNF
///
/// ```text
/// If-Range = entity-tag / HTTP-date
/// ```
///
/// # Example values
///
/// * `Sat, 29 Oct 1994 19:43:31 GMT`
/// * `\"xyzzy\"`
///
/// # Examples
///
/// ```
/// use hyper::header::{Headers, IfRange, EntityTag};
///
/// let mut headers = Headers::new();
/// headers.set(IfRange::EntityTag(EntityTag::new(false, "xyzzy".to_owned())));
/// ```
///
/// ```
/// use hyper::header::{Headers, IfRange};
/// use std::time::{SystemTime, Duration};
///
/// let mut headers = Headers::new();
/// let fetched = SystemTime::now() - Duration::from_secs(60 * 60 * 24);
/// headers.set(IfRange::Date(fetched.into()));
/// ```
#[derive(Clone, Debug, PartialEq)]
pub enum IfRange {
    /// The entity-tag the client has of the resource
    EntityTag(EntityTag),
    /// The date when the client retrieved the resource
    Date(HttpDate),
}

impl Header for IfRange {
    fn header_name() -> &'static str {
        static NAME: &'static str = "If-Range";
        NAME
    }
    fn parse_header(raw: &Raw) -> ::Result<IfRange> {
        let etag: ::Result<EntityTag> = header::parsing::from_one_raw_str(raw);
        if let Ok(etag) = etag {
            return Ok(IfRange::EntityTag(etag));
        }
        let date: ::Result<HttpDate> = header::parsing::from_one_raw_str(raw);
        if let Ok(date) = date {
            return Ok(IfRange::Date(date));
        }
        Err(::Error::Header)
    }

    fn fmt_header(&self, f: &mut ::header::Formatter) -> ::std::fmt::Result {
        f.fmt_line(self)
    }
}

impl Display for IfRange {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            IfRange::EntityTag(ref x) => Display::fmt(x, f),
            IfRange::Date(ref x) => Display::fmt(x, f),
        }
    }
}

#[cfg(test)]
mod test_if_range {
    use std::str;
    use header::*;
    use super::IfRange as HeaderField;
    test_header!(test1, vec![b"Sat, 29 Oct 1994 19:43:31 GMT"]);
    test_header!(test2, vec![b"\"xyzzy\""]);
    test_header!(test3, vec![b"this-is-invalid"], None::<IfRange>);
}
