use header::{EntityTag, Header, HeaderFormat};
use header::parsing::{from_comma_delimited, fmt_comma_delimited, from_one_raw_str};
use std::fmt;

/// The `If-Match` header
///
/// The `If-Match` request-header field is used with a method to make
/// it conditional.  The client provides a list of entity tags, and
/// the request is only executed if one of those tags matches the
/// current entity.
///
/// See http://www.w3.org/Protocols/rfc2616/rfc2616-sec14.html#sec14.24
#[derive(Clone, PartialEq, Debug)]
pub enum IfMatch {
    /// This corresponds to '*'.
    Any,
    /// The header field names which will influence the response representation.
    EntityTags(Vec<EntityTag>)
}

impl Header for IfMatch {
    fn header_name() -> &'static str {
        "If-Match"
    }

    fn parse_header(raw: &[Vec<u8>]) -> Option<IfMatch> {
        from_one_raw_str(raw).and_then(|s: String| {
            let slice = &s[..];
            match slice {
                "" => None,
                "*" => Some(IfMatch::Any),
                _ => from_comma_delimited(raw).map(IfMatch::EntityTags),
            }
        })
    }
}

impl HeaderFormat for IfMatch {
    fn fmt_header(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            IfMatch::Any => write!(fmt, "*"),
            IfMatch::EntityTags(ref fields) => fmt_comma_delimited(fmt, &fields[..])
        }
    }
}

#[test]
fn test_parse_header() {
    {
        let a: IfMatch = Header::parse_header(
        [b"*".to_vec()].as_slice()).unwrap();
        assert_eq!(a, IfMatch::Any);
    }
    {
        let a: IfMatch = Header::parse_header(
            [b"\"xyzzy\", \"r2d2xxxx\", \"c3piozzzz\"".to_vec()].as_slice()).unwrap();
        let b = IfMatch::EntityTags(
            vec![EntityTag{weak:false, tag: "xyzzy".to_string()},
                 EntityTag{weak:false, tag: "r2d2xxxx".to_string()},
                 EntityTag{weak:false, tag: "c3piozzzz".to_string()}]);
        assert_eq!(a, b);
    }
}

bench_header!(star, IfMatch, { vec![b"*".to_vec()] });
bench_header!(single , IfMatch, { vec![b"\"xyzzy\"".to_vec()] });
bench_header!(multi, IfMatch,
              { vec![b"\"xyzzy\", \"r2d2xxxx\", \"c3piozzzz\"".to_vec()] });
