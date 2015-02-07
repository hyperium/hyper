use header::{EntityTag, Header, HeaderFormat};
use header::parsing;
use std::fmt;
use std::str::FromStr;

/// An entity tag match specification
#[derive(PartialEq, Clone, Debug)]
pub enum EntityTagMatch{
    EntityTags(Vec<EntityTag>),
    Star
}

/// The `If-Match` header
///
/// The `If-Match` request-header field is used with a method to make
/// it conditional.  The client provides a list of entity tags, and
/// the request is only executed if one of those tags matches the
/// current entity.
///
/// See http://www.w3.org/Protocols/rfc2616/rfc2616-sec14.html#sec14.24
#[derive(Clone, PartialEq, Debug)]
pub struct IfMatch(pub EntityTagMatch);

deref!(IfMatch => EntityTagMatch);

impl Header for IfMatch {
    fn header_name() -> &'static str {
        "If-Match"
    }

    fn parse_header(raw: &[Vec<u8>]) -> Option<IfMatch> {
        if raw.get(0).and_then(|x| x.get(0).map(|y| y == &b'*')).unwrap_or(false) {
            Some(IfMatch(EntityTagMatch::Star))
        } else {
            match parsing::from_comma_delimited(raw).map(EntityTagMatch::EntityTags) {
                Some(tag) => Some(IfMatch(tag)),
                None => None
            }
        }
    }
}

impl HeaderFormat for IfMatch {
    fn fmt_header(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match self.0 {
            EntityTagMatch::Star => write!(fmt, "{}", "*"),
            EntityTagMatch::EntityTags(ref x) =>
                parsing::fmt_comma_delimited(fmt, &x[])
        }
    }
}

impl FromStr for IfMatch {
    type Err = ();
    fn from_str(s: &str) -> Result<IfMatch, ()> {
        if s.trim()=="*" {
            Ok(IfMatch(EntityTagMatch::Star))
        } else {
            parsing::from_comma_delimited(&[s.as_bytes().to_vec()])
                .map(|x| Ok(IfMatch(EntityTagMatch::EntityTags(x))))
                .unwrap_or(Err(()))
        }
    }
}

#[test]
fn test_parse_header() {
    {
        let a: IfMatch = Header::parse_header(
        [b"*".to_vec()].as_slice()).unwrap();
        let b = IfMatch(EntityTagMatch::Star);
        assert_eq!(a, b);
    }
    {
        let a: IfMatch = Header::parse_header(
            [b"\"xyzzy\", \"r2d2xxxx\", \"c3piozzzz\"".to_vec()].as_slice()).unwrap();
        let b = IfMatch(EntityTagMatch::EntityTags(
            vec![EntityTag{weak:false, tag: "xyzzy".to_string()},
                 EntityTag{weak:false, tag: "r2d2xxxx".to_string()},
                 EntityTag{weak:false, tag: "c3piozzzz".to_string()}]));
        assert_eq!(a, b);
    }
}

bench_header!(star, IfMatch, { vec![b"*".to_vec()] });
bench_header!(single , IfMatch, { vec![b"\"xyzzy\"".to_vec()] });
bench_header!(multi, IfMatch,
              { vec![b"\"xyzzy\", \"r2d2xxxx\", \"c3piozzzz\"".to_vec()] });
