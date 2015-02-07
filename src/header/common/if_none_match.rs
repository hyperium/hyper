use header::{Header, HeaderFormat, EntityTag};
use header::parsing::{from_comma_delimited, fmt_comma_delimited, from_one_raw_str};
use std::fmt::{self};

/// The `If-None-Match` header defined by HTTP/1.1.
///
/// The "If-None-Match" header field makes the request method conditional
/// on a recipient cache or origin server either not having any current
/// representation of the target resource, when the field-value is "*",
/// or having a selected representation with an entity-tag that does not
/// match any of those listed in the field-value.
///
/// A recipient MUST use the weak comparison function when comparing
/// entity-tags for If-None-Match (Section 2.3.2), since weak entity-tags
/// can be used for cache validation even if there have been changes to
/// the representation data.
///
/// Spec: https://tools.ietf.org/html/rfc7232#section-3.2

/// The `If-None-Match` header field.
#[derive(Clone, PartialEq, Debug)]
pub enum IfNoneMatch {
    /// This corresponds to '*'.
    Any,
    /// The header field names which will influence the response representation.
    EntityTags(Vec<EntityTag>)
}

impl Header for IfNoneMatch {
    fn header_name() -> &'static str {
        "If-None-Match"
    }

    fn parse_header(raw: &[Vec<u8>]) -> Option<IfNoneMatch> {
        from_one_raw_str(raw).and_then(|s: String| {
            let slice = &s[];
            match slice {
                "" => None,
                "*" => Some(IfNoneMatch::Any),
                _ => from_comma_delimited(raw).map(|vec| IfNoneMatch::EntityTags(vec)),
            }
        })
    }
}

impl HeaderFormat for IfNoneMatch {
    fn fmt_header(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            IfNoneMatch::Any => { write!(fmt, "*") }
            IfNoneMatch::EntityTags(ref fields) => { fmt_comma_delimited(fmt, &fields[]) }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::IfNoneMatch;
    use header::Header;
    use header::EntityTag;

    #[test]
    fn test_if_none_match() {
        let mut if_none_match: Option<IfNoneMatch>;

        if_none_match = Header::parse_header([b"*".to_vec()].as_slice());
        assert_eq!(if_none_match, Some(IfNoneMatch::Any));

        if_none_match = Header::parse_header([b"\"foobar\", W/\"weak-etag\"".to_vec()].as_slice());
        let mut entities: Vec<EntityTag> = Vec::new();
        let foobar_etag = EntityTag {
            weak: false,
            tag: "foobar".to_string()
        };
        let weak_etag = EntityTag {
            weak: true,
            tag: "weak-etag".to_string()
        };
        entities.push(foobar_etag);
        entities.push(weak_etag);
        assert_eq!(if_none_match, Some(IfNoneMatch::EntityTags(entities)));
    }
}

bench_header!(bench, IfNoneMatch, { vec![b"W/\"nonemptytag\"".to_vec()] });
